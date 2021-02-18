use std::io::{Read, Write};
use trust_dns_client::op::Header;
use trust_dns_client::proto::serialize::binary::BinDecodable;

use super::{MessageOrHeader, Worker, HEADER_SIZE};
use crate::runner::consumer::ResponseConsumer;
use crate::runner::producer::PacketGeneratorStatus;
use crate::runner::report::StatusStore;
use crate::runner::QueryProducer;
use crate::utils::Argument;
use mio::event::Event;
use mio::net::{TcpKeepalive, TcpSocket};
use mio::{net::TcpStream, Events, Interest, Poll, Token};
use std::collections::HashMap;
use std::ops::Add;
use std::thread::sleep;
use std::time::Duration;

pub struct TCPWorker {
    arguments: Argument,
    poll: Poll,
    events: Events,
    sockets: Vec<TcpStream>,
    // server_port: String,
}

enum SocketStatus {
    Close,
    Success,
    WouldBlock,
    Err(String),
}

impl TCPWorker {
    fn write_data(connection: &mut TcpStream, event: &Event, data: &[u8]) -> SocketStatus {
        match connection.write(data) {
            Ok(n) if n < data.len() => {
                return SocketStatus::Err("partial write fail".to_string());
            }
            Ok(_) => SocketStatus::Success,
            Err(ref err) if would_block(err) => SocketStatus::WouldBlock,
            Err(ref err) if interrupted(err) => {
                return TCPWorker::write_data(connection, event, data);
            }
            Err(err) => SocketStatus::Err(err.to_string()),
        }
    }

    fn read_data(connection: &mut TcpStream, data: &mut [u8]) -> SocketStatus {
        let mut received_data = vec![0; 512];
        let mut bytes_read = 0;
        loop {
            match connection.read(&mut received_data[bytes_read..]) {
                Ok(0) => {
                    break;
                }
                Ok(n) => {
                    bytes_read += n;
                    if bytes_read == received_data.len() {
                        received_data.resize(received_data.len() + 512, 0);
                    }
                }
                Err(ref err) if would_block(err) => {
                    break;
                }
                Err(ref err) if interrupted(err) => {
                    continue;
                }
                // Other errors we'll consider fatal.
                Err(err) => {
                    return SocketStatus::Err(err.to_string());
                }
            };
        }
        if bytes_read != 0 {
            data.copy_from_slice(&received_data[0..data.len()]);
            SocketStatus::Success
        } else {
            SocketStatus::Close
        }
    }
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
        let mut time_store = HashMap::new();
        let mut dns_packet = vec![0u8; 14];
        'outer: loop {
            for event in self.events.iter() {
                let token = event.token();
                let ref mut connection = self.sockets[token.0];
                if event.is_writable() {
                    debug!("socket {} is writable", token.0);
                    match producer.retrieve() {
                        PacketGeneratorStatus::Success(data, qtype) => {
                            let key = ((data[2] as u16) << 8) | (data[3] as u16);
                            time_store.insert(key, chrono::Utc::now().timestamp_nanos());
                            match TCPWorker::write_data(connection, event, data) {
                                SocketStatus::Success => {
                                    send_counter += 1;
                                    producer.store.update_query(qtype);
                                    stop_sender_timer = std::time::SystemTime::now();
                                    debug!(
                                        "send success receive = {},  current = {}",
                                        receive_counter, send_counter
                                    );
                                    self.poll
                                        .registry()
                                        .reregister(connection, token, Interest::READABLE)
                                        .expect("reregister fail");
                                }
                                SocketStatus::WouldBlock => {
                                    debug!("receive would block");
                                    producer.return_back();
                                }
                                SocketStatus::Err(e) => {
                                    debug!("send error: {}", e);
                                    producer.return_back();
                                }
                                _ => {
                                    debug!("send error with no clue");
                                    producer.return_back();
                                }
                            };
                        }
                        PacketGeneratorStatus::Wait(wait) => {
                            debug!("wait for next ticker");
                            self.poll
                                .registry()
                                .reregister(connection, token, Interest::WRITABLE)
                                .expect("reregister fail");
                            // sleep(std::time::Duration::from_nanos(wait));
                        }
                        PacketGeneratorStatus::Stop => {
                            debug!("receive stop signal");
                        }
                    }
                }
                if event.is_readable() {
                    debug!("socket {} is readable", token.0);
                    let result = TCPWorker::read_data(connection, dns_packet.as_mut_slice());
                    match result {
                        SocketStatus::Success => {
                            self.poll
                                .registry()
                                .reregister(connection, token, Interest::WRITABLE)
                                .expect("reregister fail");
                            let key = ((dns_packet[2] as u16) << 8) | (dns_packet[3] as u16);
                            let duration = match time_store.get(&key) {
                                Some(start) => {
                                    (chrono::Utc::now().timestamp_nanos() - start) as f64
                                        / 1000000000.0
                                }
                                _ => 0.0,
                            };
                            debug!(
                                "receive success receive = {},  current = {}",
                                receive_counter, send_counter
                            );
                            if let Ok(message) = Header::from_bytes(&dns_packet[2..HEADER_SIZE + 2])
                            {
                                consumer.receive(&MessageOrHeader::Header((message, duration)));
                                receive_counter += 1;
                            } else {
                                error!("parse dns message error");
                                continue;
                            }
                        }
                        _ => {
                            error!("read socket fail")
                        }
                    }
                }
                if (max_send > 0 && (receive_counter == max_send))
                    || stop_sender_timer.elapsed().unwrap() > std::time::Duration::from_secs(5)
                {
                    debug!(
                        "should break loop {} {} cpu={}",
                        send_counter, receive_counter, id
                    );
                    break 'outer;
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
            // let socket = TcpSocket::new_v4().unwrap();
            // let keepalive = TcpKeepalive::default()
            //     .with_time(Duration::from_secs(4));
            // socket.set_keepalive_params(keepalive);
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
                        .register(&mut stream, Token(i), Interest::WRITABLE)
                        .expect("registr event fail");
                    debug!("register Interest::WRITABLE for socket {}", i);
                    // stream.set_keepalive();
                    sockets.push(stream);
                }
            }
        }
        Box::new(TCPWorker {
            arguments: arguments.clone(),
            poll,
            events,
            sockets,
            // server_port,
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

fn would_block(err: &std::io::Error) -> bool {
    err.kind() == std::io::ErrorKind::WouldBlock
}

fn interrupted(err: &std::io::Error) -> bool {
    err.kind() == std::io::ErrorKind::Interrupted
}
