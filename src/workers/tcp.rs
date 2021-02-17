use std::io::{Read, Write};
use trust_dns_client::op::Header;
use trust_dns_client::proto::serialize::binary::BinDecodable;

use super::{MessageOrHeader, Worker, HEADER_SIZE};
use crate::runner::consumer::ResponseConsumer;
use crate::runner::producer::PacketGeneratorStatus;
use crate::runner::report::StatusStore;
use crate::runner::QueryProducer;
use crate::utils::Argument;
use mio::{net::TcpStream, Events, Interest, Poll, Token};
use std::collections::HashMap;
use std::ops::Add;
use std::thread::sleep;

pub struct TCPWorker {
    arguments: Argument,
    poll: Poll,
    events: Events,
    sockets: Vec<TcpStream>,
}

impl Worker for TCPWorker {
    fn run(
        &mut self,
        id: usize,
        sender: crossbeam_channel::Sender<(StatusStore, StatusStore)>,
    ) -> (StatusStore, StatusStore) {
        let arguments = self.arguments.clone();
        let interval = arguments.output_interval as u64;
        let mut next_status_send =
            std::time::SystemTime::now().add(std::time::Duration::from_secs(interval));
        let mut producer = QueryProducer::new(arguments.clone());
        let mut consumer = ResponseConsumer::new();
        #[allow(unused_assignments)]
        let mut stop_sender_timer = std::time::SystemTime::now();
        let max_send = arguments.max as u64;
        let mut send_counter: u64 = 0;
        let mut receive_counter: u64 = 0;
        let start = std::time::SystemTime::now();
        if let Err(e) = self.poll.poll(&mut self.events, None) {
            error!("poll event fail: {}", e.to_string());
        };
        let sockets_number = self.sockets.len();
        let mut register_sockets = vec![false; sockets_number];
        let mut time_store = HashMap::new();

        'outer: loop {
            for event in self.events.iter() {
                match event.token() {
                    Token(i) if event.is_writable() => {
                        match producer.retrieve() {
                            PacketGeneratorStatus::Success(data, qtype) => {
                                let key = ((data[2] as u16) << 8) | (data[3] as u16);
                                time_store.insert(key, chrono::Utc::now().timestamp_nanos());
                                if let Err(e) = self.sockets[i].write(data) {
                                    error!("send error : {}", e);
                                }
                                send_counter += 1;
                                producer.store.update_query(qtype);
                                debug!(
                                    "send success in socket {} current={} cpu={}",
                                    i, send_counter, id
                                );
                                stop_sender_timer = std::time::SystemTime::now();
                            }
                            PacketGeneratorStatus::Wait(wait) => {
                                sleep(std::time::Duration::from_nanos(wait));
                            }
                            PacketGeneratorStatus::Stop => {
                                // debug!("receive stop signal");
                            }
                        };
                        register_sockets[i] = true;
                    }
                    Token(i) if event.is_readable() => {
                        // Read Event
                        let mut buffer = vec![0; HEADER_SIZE + 2];
                        if let Ok(size) = self.sockets[i].read(&mut buffer) {
                            debug!(
                                "receive success in socket {} current={} cpu={}",
                                i, receive_counter, id
                            );
                            let key = ((buffer[2] as u16) << 8) | (buffer[3] as u16);
                            let duration = match time_store.get(&key) {
                                Some(start) => {
                                    (chrono::Utc::now().timestamp_nanos() - start) as f64
                                        / 1000000000.0
                                }
                                _ => 0.0,
                            };
                            register_sockets[i] = true;
                            if let Ok(message) = Header::from_bytes(&buffer[2..size]) {
                                consumer.receive(&MessageOrHeader::Header((message, duration)));
                            } else {
                                error!("parse dns message error");
                            }
                            receive_counter += 1;
                        }
                        if (max_send > 0 && (receive_counter == max_send))
                            || stop_sender_timer.elapsed().unwrap()
                                > std::time::Duration::from_secs(5)
                        {
                            debug!(
                                "should break loop {} {} cpu={}",
                                send_counter, receive_counter, id
                            );
                            break 'outer;
                        }
                    }
                    _ => {
                        warn!("Got event for unexpected token: {:?}", event);
                    }
                }
            }
            for i in 0..sockets_number {
                if register_sockets[i] == true {
                    self.poll
                        .registry()
                        .reregister(
                            &mut self.sockets[i],
                            Token(i),
                            Interest::WRITABLE | Interest::READABLE,
                        )
                        .expect("re register socket fail");
                    register_sockets[i] = false;
                }
            }

            if let Err(e) = self.poll.poll(&mut self.events, None) {
                error!("poll event fail: {}", e.to_string());
                break;
            }
            if interval != 0 {
                let now = std::time::SystemTime::now();
                if now >= next_status_send {
                    producer
                        .store
                        .set_send_duration(now.duration_since(start).unwrap());
                    consumer.store.set_receive_total(receive_counter);
                    consumer.update_report();
                    if let Err(err) = sender.send((producer.store.clone(), consumer.store.clone()))
                    {
                        error!("send interval status fail: {:?}", err)
                    }
                    next_status_send = now.add(std::time::Duration::from_secs(interval));
                }
            }
        }
        std::mem::drop(sender);
        producer
            .store
            .set_send_duration(stop_sender_timer.duration_since(start).unwrap());
        consumer.store.set_receive_total(receive_counter);
        consumer.receive(&MessageOrHeader::End);
        for socket in self.sockets.iter_mut() {
            self.poll
                .registry()
                .deregister(socket)
                .expect("deregister socket fail");
        }
        (producer.store, consumer.store)
    }
}

impl TCPWorker {
    pub fn new(arguments: Argument) -> Box<dyn Worker> {
        let server_port = format!("{}:{}", arguments.server, arguments.port);
        let poll = Poll::new().expect("create async poll fail");
        let events = Events::with_capacity(1024);
        let mut sockets = vec![];

        for i in 0..arguments.client {
            match TcpStream::connect(
                server_port
                    .parse()
                    .expect("server ip and port can't be connect success"),
            ) {
                Err(e) => {
                    error!("{}", e.to_string());
                    continue;
                }
                Ok(mut stream) => {
                    poll.registry()
                        .register(
                            &mut stream,
                            Token(i),
                            Interest::READABLE | Interest::WRITABLE,
                        )
                        .expect("registr event fail");
                    debug!("register for socket {}", i);
                    sockets.push(stream);
                }
            }
        }
        Box::new(TCPWorker {
            arguments: arguments.clone(),
            poll,
            events,
            sockets,
        })
    }

    // pub fn new(
    //     arguments: Argument,
    //     receiver: Arc<Mutex<Receiver<Vec<u8>>>>,
    //     result_sender: Sender<MessageOrHeader>,
    // ) -> TCPWorker {
    //     let rx = receiver.clone();
    //     let server_port = format!("{}:{}", arguments.server, arguments.port);
    //     let check_all_message = arguments.check_all_message;
    //
    //     // stream.set_nonblocking(true).expect("set tcp unblock fail");
    //     let thread = std::thread::spawn(move || {
    //         // TODO : tcp source ip setting
    //         let mut stream = TcpStream::connect(server_port.clone())
    //             .expect(format!("unable to connect to server :{}", server_port).as_str());
    //
    //         if let Err(e) = stream.set_read_timeout(Some(std::time::Duration::from_secs(
    //             arguments.timeout as u64,
    //         ))) {
    //             error!("read_timeout {:?}", e);
    //         }
    //         loop {
    //             let data = match rx.lock().unwrap().recv() {
    //                 Ok(data) => data,
    //                 Err(_) => {
    //                     break;
    //                 }
    //             };
    //             debug!("send {:?}", data.as_slice());
    //             let start = Instant::now();
    //             match stream.write(data.as_slice()) {
    //                 Err(e) => {
    //                     println!("send error : {}", e);
    //                 }
    //                 Ok(_) => {}
    //             };
    //             let mut length = vec![0u8; 2];
    //             if let Err(_e) = stream.read_exact(&mut length) {
    //                 continue;
    //             };
    //             let size = (length[0] as usize) << 8 | length[1] as usize;
    //             let mut data = vec![0; size];
    //             if let Err(e) = stream.read_exact(&mut data) {
    //                 debug!("receive error: {}", e.to_string())
    //             }
    //             if check_all_message == true {
    //                 if let Ok(message) = Message::from_bytes(data.as_slice()) {
    //                     if let Err(e) = result_sender.send(MessageOrHeader::Message((
    //                         message,
    //                         start.elapsed().as_secs_f64(),
    //                     ))) {
    //                         error!("send packet: {}", e.to_string())
    //                     };
    //                 }
    //             } else {
    //                 if let Ok(message) = Header::from_bytes(&data[0..HEADER_SIZE]) {
    //                     let code = message.response_code();
    //                     if code != trust_dns_client::proto::op::ResponseCode::NoError.low() {
    //                         println!("{} --- {:?}", size, message)
    //                     }
    //                     if let Err(e) = result_sender.send(MessageOrHeader::Header((
    //                         message,
    //                         start.elapsed().as_secs_f64(),
    //                     ))) {
    //                         error!("send packet: {}", e.to_string())
    //                     };
    //                 }
    //             }
    //         }
    //         debug!("tcp worker thread exit success");
    //         drop(result_sender);
    //     });
    //     TCPWorker {
    //         write_thread: Some(thread),
    //     }
    // }
}
