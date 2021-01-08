use super::{MessageOrHeader, Worker, HEADER_SIZE};
use crate::arguments::Argument;
use crossbeam_channel::{Receiver, Sender};
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use trust_dns_client::op::{Header, Message};
use trust_dns_client::proto::serialize::binary::BinDecodable;

pub struct UDPWorker {
    write_thread: Option<std::thread::JoinHandle<()>>,
}
impl Worker for UDPWorker {
    fn block(&mut self) {
        if let Some(handler) = self.write_thread.take() {
            handler.join().expect("fail to join doh thread");
        }
    }
}

impl UDPWorker {
    pub fn new(
        arguments: Argument,
        receiver: Arc<Mutex<Receiver<Vec<u8>>>>,
        result_sender: Sender<MessageOrHeader>,
    ) -> UDPWorker {
        let rx = receiver.clone();
        let server_port = format!("{}:{}", arguments.server, arguments.port);
        let source_ip_addr = format!("{}:0", arguments.source);
        let socket = UdpSocket::bind(source_ip_addr).unwrap();

        socket
            .connect(server_port)
            .expect("unable to connect to server");
        // socket.set_nonblocking(true).expect("set udp unblock fail");
        let edns_size_local = arguments.edns_size as usize;
        let check_all_message = arguments.check_all_message;

        let thread = std::thread::spawn(move || {
            loop {
                // TODO: how about each thread has own producer?
                let data = match rx.lock().unwrap().recv() {
                    Ok(data) => data,
                    Err(_) => {
                        break;
                    }
                };
                debug!("send {:?}", data.as_slice());
                let start = Instant::now();
                if let Err(e) = socket.send(data.as_slice()) {
                    error!("send error : {}", e);
                };

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
                    }
                }
            }
            debug!("udp worker thread exit success");
            drop(result_sender);
        });
        UDPWorker {
            write_thread: Some(thread),
        }
    }
}
