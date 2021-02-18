use super::{MessageOrHeader, Worker, HEADER_SIZE};
use crate::runner::consumer::ResponseConsumer;
use crate::runner::report::StatusStore;
use crate::runner::{producer::PacketGeneratorStatus, QueryProducer};
use crate::utils::Argument;

use mio::net::UdpSocket;
use mio::{Events, Interest, Poll, Token};
use std::collections::HashMap;
use std::ops::Add;
use std::thread::sleep;
use trust_dns_client::op::Header;
use trust_dns_client::proto::serialize::binary::BinDecodable;

pub struct UDPWorker {
    arguments: Argument,
    poll: Poll,
    events: Events,
    sockets: Vec<UdpSocket>,
}

impl Worker for UDPWorker {
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
                // debug!("loop for events");
                match event.token() {
                    Token(i) if event.is_writable() => {
                        match producer.retrieve() {
                            PacketGeneratorStatus::Success(data, qtype) => {
                                let key = ((data[0] as u16) << 8) | (data[1] as u16);
                                time_store.insert(key, chrono::Utc::now().timestamp_nanos());
                                if let Err(e) = self.sockets[i].send(data) {
                                    error!("send error : {}", e);
                                    producer.return_back();
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
                        let mut buffer = vec![0; HEADER_SIZE];
                        if let Ok(size) = self.sockets[i].recv(&mut buffer) {
                            debug!(
                                "receive success in socket {} current={} cpu={}",
                                i, receive_counter, id
                            );
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

impl UDPWorker {
    pub fn new(arguments: Argument) -> Box<dyn Worker> {
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

        Box::new(UDPWorker {
            arguments: arguments.clone(),
            poll,
            events,
            sockets,
        })
    }
}
