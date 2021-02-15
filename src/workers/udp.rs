use super::{MessageOrHeader, Worker, HEADER_SIZE};
use crate::runner::consumer::ResponseConsumer;
use crate::runner::report::StatusStore;
use crate::runner::{producer::PacketGeneratorStatus, QueryProducer};
use crate::utils::Argument;

use mio::net::UdpSocket;
use mio::{Events, Interest, Poll, Token};
use std::collections::HashMap;
use std::thread::sleep;
use trust_dns_client::op::{Header, Message};
use trust_dns_client::proto::serialize::binary::BinDecodable;

pub struct UDPWorker {
    arguments: Argument,
    poll: Poll,
    events: Events,
    sockets: Vec<UdpSocket>,
}

impl Worker for UDPWorker {
    fn run(&mut self) -> (StatusStore, StatusStore) {
        let arguments = self.arguments.clone();
        let edns_size_local = self.arguments.edns_size as usize;
        let check_all_message = self.arguments.check_all_message;
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
                // debug!("loop for events");
                match event.token() {
                    Token(i) if event.is_writable() => {
                        match producer.retrieve() {
                            PacketGeneratorStatus::Success(data, qtype) => {
                                let key = ((data[0] as u16) << 8) | (data[1] as u16);
                                time_store.insert(key, chrono::Utc::now().timestamp_nanos());
                                if let Err(e) = self.sockets[i].send(data) {
                                    error!("send error : {}", e);
                                }
                                send_counter += 1;
                                producer.store.update_query(qtype);
                                debug!("send success in socket {} {}", i, send_counter);
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
                        if check_all_message == true {
                            let mut buffer = vec![0; edns_size_local];
                            if let Ok(size) = self.sockets[i].recv(&mut buffer) {
                                debug!("receive success in socket {} {}", i, receive_counter);
                                let key = ((buffer[0] as u16) << 8) | (buffer[1] as u16);
                                let duration = match time_store.get(&key) {
                                    Some(start) => {
                                        (chrono::Utc::now().timestamp_nanos() - start) as f64
                                            / 1000000000.0
                                    }
                                    _ => 0.0,
                                };
                                register_sockets[i] = true;
                                if let Ok(message) = Message::from_bytes(&buffer[..size]) {
                                    consumer
                                        .receive(&MessageOrHeader::Message((message, duration)));
                                } else {
                                    error!("parse dns message error");
                                }
                                receive_counter += 1;
                            }
                        } else {
                            let mut buffer = vec![0; HEADER_SIZE];
                            if let Ok(size) = self.sockets[i].recv(&mut buffer) {
                                debug!("receive success in socket {} {}", i, receive_counter);
                                let key = ((buffer[0] as u16) << 8) | (buffer[1] as u16);
                                let duration = match time_store.get(&key) {
                                    Some(start) => {
                                        (chrono::Utc::now().timestamp_nanos() - start) as f64
                                            / 1000000000.0
                                    }
                                    _ => 0.0,
                                };
                                register_sockets[i] = true;
                                if let Ok(message) = Header::from_bytes(&buffer[..size]) {
                                    consumer.receive(&MessageOrHeader::Header((message, duration)));
                                } else {
                                    error!("parse dns message error");
                                }
                                receive_counter += 1;
                            }
                        }
                        if (max_send > 0 && (receive_counter == max_send))
                            || stop_sender_timer.elapsed().unwrap()
                                > std::time::Duration::from_secs(5)
                        {
                            debug!("should break loop {} {}", send_counter, receive_counter);
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
        }
        debug!("break the event loop");
        producer
            .store
            .set_send_duration(stop_sender_timer.duration_since(start).unwrap());
        consumer.store.set_receive_total(receive_counter);
        consumer.receive(&MessageOrHeader::End);
        debug!(
            "producer = {:?} \n consumer = {:?}",
            producer.store, consumer.store
        );
        for socket in self.sockets.iter_mut() {
            self.poll
                .registry()
                .deregister(socket)
                .expect("deregister socket fail");
        }
        (producer.store, consumer.store)
    }
}

impl UDPWorker {
    pub fn new(arguments: Argument) -> UDPWorker {
        let server_port = format!("{}:{}", arguments.server, arguments.port);
        let source_ip_addr = format!("{}:0", arguments.source);
        let poll = Poll::new().expect("create async poll fail");
        let events = Events::with_capacity(1024);
        let mut sockets = vec![];

        for i in 0..arguments.client {
            let mut socket = UdpSocket::bind(
                source_ip_addr
                    .parse()
                    .expect("source ip addr is not set correct"),
            )
            .unwrap();
            if let Err(e) = socket.connect(
                server_port
                    .parse()
                    .expect("server ip and port can't be connect success"),
            ) {
                error!("{}", e.to_string());
                continue;
            }
            poll.registry()
                .register(
                    &mut socket,
                    Token(i),
                    Interest::READABLE | Interest::WRITABLE,
                )
                .expect("registr event fail");
            debug!("register for socket {}", i);
            sockets.push(socket);
        }

        UDPWorker {
            arguments: arguments.clone(),
            poll,
            events,
            sockets,
        }
    }
}
