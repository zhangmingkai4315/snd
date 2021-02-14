use super::{MessageOrHeader, Worker, HEADER_SIZE};
use crate::runner::report::QueryStatusStore;
use crate::runner::{producer::PacketGeneratorStatus, QueryProducer};
use crate::utils::Argument;
use chrono::Duration;
use crossbeam_channel::{Receiver, Sender};
use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::thread::{sleep, JoinHandle};
use std::time::Instant;
use trust_dns_client::op::{Header, Message};
use trust_dns_client::proto::serialize::binary::BinDecodable;

// send_counter: AtomicU64,
// static mut RECEIVE_COUNTER: AtomicU64 = AtomicU64::new(0);
// static mut SEND_COUNTER: AtomicU64 = AtomicU64::new(0);
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
        // socket.set_nonblocking(true);
        socket
            .connect(server_port.clone())
            .expect("unable to connect to server");
        let status_sender = self.status_sender.clone();
        let result_sender = self.result_sender.clone();

        std::thread::spawn(move || {
            let mut producer = QueryProducer::new(arguments);
            let mut receive_counter: usize = 0;
            let mut send_counter: usize = 0;
            let mut start = std::time::SystemTime::now();
            let mut stop_sender_timer = std::time::SystemTime::now();
            loop {
                match producer.retrieve() {
                    PacketGeneratorStatus::Success(data, qtype) => {
                        start = std::time::SystemTime::now();
                        if let Err(e) = socket.send(data) {
                            error!("send error : {}", e);
                        }
                        producer.store.update_query(qtype);
                        send_counter += 1;
                    }
                    PacketGeneratorStatus::Wait(wait) => {
                        sleep(std::time::Duration::from_nanos(wait));
                        continue;
                    }
                    PacketGeneratorStatus::Stop => {
                        stop_sender_timer = std::time::SystemTime::now();
                        break;
                    }
                };
                if check_all_message == true {
                    let mut buffer = vec![0; edns_size_local];
                    if let Ok(size) = socket.recv(&mut buffer) {
                        if let Ok(message) = Message::from_bytes(&buffer[..size]) {
                            if let Err(e) = result_sender.send(MessageOrHeader::Message((
                                message,
                                start.elapsed().unwrap().as_secs_f64(),
                            ))) {
                                error!("send packet: {:?}", e);
                            }
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
                                start.elapsed().unwrap().as_secs_f64(),
                            ))) {
                                error!("send packet: {:?}", e);
                            }
                        } else {
                            error!("parse dns message error");
                        }
                        receive_counter += 1;
                    }
                }
            }

            producer.store.set_receive_total(receive_counter);
            debug!("{:?}", producer.store);
            status_sender.send(producer.store);
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
