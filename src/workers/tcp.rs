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
use mio::{net::TcpStream, Events, Interest, Poll, Token};
use std::collections::HashMap;
use std::ops::Add;

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
                    if err.kind() == std::io::ErrorKind::ConnectionReset {
                        return SocketStatus::Close;
                    }
                    debug!("read data error: {}", err.to_string());
                    return SocketStatus::Err(err.to_string());
                }
            };
        }
        if bytes_read != 0 {
            data.copy_from_slice(&received_data[0..data.len()]);
            SocketStatus::Success
        } else {
            debug!("read zero byte");
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
                    // debug!("socket {} is writable", token.0);
                    match producer.retrieve() {
                        PacketGeneratorStatus::Success(data, qtype) => {
                            let key = ((data[2] as u16) << 8) | (data[3] as u16);
                            // sample 1/10
                            if key % 10 == 1 {
                                time_store.insert(key, std::time::SystemTime::now());
                            }
                            match TCPWorker::write_data(connection, event, data) {
                                SocketStatus::Success => {
                                    send_counter += 1;
                                    producer.store.update_query(qtype);
                                    stop_sender_timer = std::time::SystemTime::now();
                                    //
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
                                    // debug!("receive would block");
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
                        PacketGeneratorStatus::Wait(_) => {
                            // debug!("wait for next ticker");
                            self.poll
                                .registry()
                                .reregister(connection, token, Interest::WRITABLE)
                                .expect("reregister fail");
                        }
                        PacketGeneratorStatus::Stop => {
                            debug!("receive stop signal");
                        }
                    }
                }
                if event.is_readable() {
                    let result = TCPWorker::read_data(connection, dns_packet.as_mut_slice());
                    match result {
                        SocketStatus::Success => {
                            self.poll
                                .registry()
                                .reregister(connection, token, Interest::WRITABLE)
                                .expect("reregister fail");
                            let key = ((dns_packet[2] as u16) << 8) | (dns_packet[3] as u16);
                            // sample 1/10
                            let mut duration: f64 = 0.0;
                            if key % 10 == 1 {
                                if let Some(start) = time_store.get(&key) {
                                    duration = start.elapsed().unwrap().as_secs_f64();
                                }
                            }
                            receive_counter += 1;
                            debug!(
                                "receive success receive = {},  send = {}",
                                receive_counter, send_counter
                            );
                            if let Ok(message) = Header::from_bytes(&dns_packet[2..HEADER_SIZE + 2])
                            {
                                consumer.receive(&MessageOrHeader::Header((message, duration)));
                            } else {
                                error!("parse dns message error");
                                continue;
                            }
                        }
                        SocketStatus::Close => {
                            debug!("reset socket");
                        }
                        _ => error!("read socket fail"),
                    }
                }
            }
            if (max_send > 0 && (receive_counter == max_send))
                || stop_sender_timer.elapsed().unwrap() > std::time::Duration::from_secs(5)
            {
                debug!(
                    "should break loop send = {} receive = {} cpu={}",
                    send_counter, receive_counter, id
                );
                break 'outer;
            }
            if let Err(e) = self.poll.poll(
                &mut self.events,
                Option::from(std::time::Duration::from_secs(1)),
            ) {
                error!("poll event fail: {}", e.to_string());
                break;
            }
            if interval != 0 {
                let now = std::time::SystemTime::now();
                if now >= next_status_send {
                    producer
                        .store
                        .set_send_duration(now.duration_since(start.clone()).unwrap());
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
                        .register(&mut stream, Token(i), Interest::WRITABLE)
                        .expect("registr event fail");
                    debug!("register Interest::WRITABLE for socket {}", i);
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
}

fn would_block(err: &std::io::Error) -> bool {
    err.kind() == std::io::ErrorKind::WouldBlock
}

fn interrupted(err: &std::io::Error) -> bool {
    err.kind() == std::io::ErrorKind::Interrupted
}
