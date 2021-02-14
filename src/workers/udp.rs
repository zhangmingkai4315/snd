use super::{MessageOrHeader, Worker, HEADER_SIZE};
use crate::runner::consumer::ResponseConsumer;
use crate::runner::report::StatusStore;
use crate::runner::{producer::PacketGeneratorStatus, QueryProducer};
use crate::utils::Argument;

use crossbeam_channel::Sender;
use std::net::UdpSocket;
use std::thread::{sleep, JoinHandle};
use trust_dns_client::op::{Header, Message};
use trust_dns_client::proto::serialize::binary::BinDecodable;

pub struct UDPWorker {
    arguments: Argument,
    pub status_sender: Sender<(StatusStore, StatusStore)>,
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
        let status_sender_channel = self.status_sender.clone();

        std::thread::spawn(move || {
            let mut producer = QueryProducer::new(arguments);
            let mut consumer = ResponseConsumer::new();
            #[allow(unused_assignments)]
            let mut stop_sender_timer = std::time::SystemTime::now();
            let mut receive_counter: usize = 0;
            let mut start = std::time::SystemTime::now();
            loop {
                match producer.retrieve() {
                    PacketGeneratorStatus::Success(data, qtype) => {
                        start = std::time::SystemTime::now();
                        if let Err(e) = socket.send(data) {
                            error!("send error : {}", e);
                        }
                        producer.store.update_query(qtype);
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
                            consumer.receive(&MessageOrHeader::Message((
                                message,
                                start.elapsed().unwrap().as_secs_f64(),
                            )));
                        } else {
                            error!("parse dns message error");
                        }
                        receive_counter += 1;
                    }
                } else {
                    let mut buffer = vec![0; HEADER_SIZE];
                    if let Ok(size) = socket.recv(&mut buffer) {
                        if let Ok(message) = Header::from_bytes(&buffer[..size]) {
                            consumer.receive(&MessageOrHeader::Header((
                                message,
                                start.elapsed().unwrap().as_secs_f64(),
                            )));
                        } else {
                            error!("parse dns message error");
                        }
                        receive_counter += 1;
                    }
                }
            }
            producer
                .store
                .set_send_duration(stop_sender_timer.duration_since(start).unwrap());
            consumer.store.set_receive_total(receive_counter);
            consumer.receive(&MessageOrHeader::End);
            debug!(
                "producer = {:?} \n consumer = {:?}",
                producer.store, consumer.store
            );
            if let Err(e) = status_sender_channel.send((producer.store, consumer.store)) {
                panic!("send status fail: {:?}", e)
            }
        })
    }
}

impl UDPWorker {
    pub fn new(
        arguments: Argument,
        status_sender: Sender<(StatusStore, StatusStore)>,
    ) -> UDPWorker {
        UDPWorker {
            arguments: arguments.clone(),
            status_sender,
        }
    }
}
