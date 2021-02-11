use super::{MessageOrHeader, Worker, HEADER_SIZE};
use crate::arguments::Argument;
use crate::runner::report::QueryStatusStore;
use crate::runner::{producer::PacketGeneratorStatus, QueryProducer};
use crossbeam_channel::{Receiver, Sender};
use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;
use trust_dns_client::op::{Header, Message};
use trust_dns_client::proto::serialize::binary::BinDecodable;

pub struct UDPWorker {
    arguments: Argument,
    result_sender: Sender<MessageOrHeader>,
    pub status_sender: Sender<QueryStatusStore>,
}

impl Worker for UDPWorker {
    fn run(&mut self) -> JoinHandle<()> {
        let arguments = self.arguments.clone();
        let server_port = format!("{}:{}", self.arguments.server, self.arguments.port);
        let source_ip_addr = format!("{}:0", self.arguments.source);
        let edns_size_local = self.arguments.edns_size as usize;
        let check_all_message = self.arguments.check_all_message;
        let socket = UdpSocket::bind(source_ip_addr.clone()).unwrap();
        socket.set_nonblocking(true);
        socket
            .connect(server_port.clone())
            .expect("unable to connect to server");
        let status_sender = self.status_sender.clone();
        let result_sender = self.result_sender.clone();
        debug!("create a udp worker");
        std::thread::spawn(move || {
            let mut producer = QueryProducer::new(arguments);
            let mut receive_counter: usize = 0;
            let mut send_counter: usize = 0;
            let begin_send = std::time::SystemTime::now();
            let mut end_send = std::time::SystemTime::now();

            'outer: loop {
                let data = match producer.retrieve() {
                    PacketGeneratorStatus::Success(data) => data,
                    PacketGeneratorStatus::Wait => continue,
                    PacketGeneratorStatus::Stop => {
                        debug!("receive stop signal for quit worker waiting for 10 seconds");
                        let begin = std::time::SystemTime::now();
                        loop {
                            if begin.elapsed().unwrap() > std::time::Duration::from_secs(10) {
                                break 'outer;
                            }
                            if check_all_message == true {
                                let mut buffer = vec![0; edns_size_local];
                                if let Ok(size) = socket.recv(&mut buffer) {
                                    if let Ok(message) = Message::from_bytes(&buffer[..size]) {
                                        if let Err(e) = result_sender
                                            .send(MessageOrHeader::Message((message, 0.0)))
                                        {
                                            error!("send packet: {:?}", e);
                                        };
                                    } else {
                                        error!("parse dns message error");
                                    }
                                    receive_counter += 1;
                                }
                            } else {
                                let mut buffer = vec![0; HEADER_SIZE];
                                if let Ok(size) = socket.recv(&mut buffer) {
                                    if let Ok(message) = Header::from_bytes(&buffer[..size]) {
                                        if let Err(e) = result_sender
                                            .send(MessageOrHeader::Header((message, 0.0)))
                                        {
                                            error!("send packet: {:?}", e);
                                        };
                                    } else {
                                        error!("parse dns message error");
                                    }
                                    receive_counter += 1;
                                }
                            }
                        }
                    }
                };
                let start = Instant::now();
                if let Err(e) = socket.send(data.as_slice()) {
                    error!("send error : {}", e);
                };
                send_counter += 1;
                end_send = std::time::SystemTime::now();
                if check_all_message == true {
                    let mut buffer = vec![0; edns_size_local];
                    if let Ok(size) = socket.recv(&mut buffer) {
                        if let Ok(message) = Message::from_bytes(&buffer[..size]) {
                            if let Err(e) = result_sender.send(MessageOrHeader::Message((
                                message,
                                start.elapsed().as_secs_f64(),
                            ))) {
                                error!("send packet: {:?}", e);
                            };
                        } else {
                            error!("parse dns message error");
                        }
                        receive_counter += 1;
                    }
                } else {
                    let mut buffer = vec![0; HEADER_SIZE];
                    if let Ok(size) = socket.recv(&mut buffer) {
                        if let Ok(message) = Header::from_bytes(&buffer[..size]) {
                            if let Err(e) = result_sender.send(MessageOrHeader::Header((
                                message,
                                start.elapsed().as_secs_f64(),
                            ))) {
                                error!("send packet: {:?}", e);
                            };
                        } else {
                            error!("parse dns message error");
                        }
                        receive_counter += 1;
                    }
                }
            }
            producer.store.set_query_total(send_counter);
            producer.store.set_receive_total(receive_counter);
            producer
                .store
                .set_send_duration(end_send.duration_since(begin_send).unwrap());
            status_sender.send(producer.store);
            info!("receive : {:?} ", receive_counter);
        })
    }
}

impl UDPWorker {
    pub fn new(
        arguments: Argument,
        result_sender: Sender<MessageOrHeader>,
        status_sender: Sender<QueryStatusStore>,
    ) -> UDPWorker {
        UDPWorker {
            arguments: arguments.clone(),
            result_sender,
            status_sender,
        }
    }
}
