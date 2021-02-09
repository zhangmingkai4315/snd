use super::{MessageOrHeader, Worker, HEADER_SIZE};
use crate::arguments::Argument;
use crossbeam_channel::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use trust_dns_client::op::{Header, Message};
use trust_dns_client::proto::serialize::binary::BinDecodable;
use tokio::runtime;
use tokio::net::UdpSocket;

pub struct UDPAsyncWorker {
    write_thread: Option<std::thread::JoinHandle<()>>,
}
impl Worker for UDPAsyncWorker {
    fn block(&mut self) {
        if let Some(handler) = self.write_thread.take() {
            handler.join().expect("fail to join udp thread");
        }
    }
}

impl UDPAsyncWorker {
    pub fn new(
        arguments: Argument,
        receiver: Arc<Mutex<Receiver<Vec<u8>>>>,
        result_sender: Sender<MessageOrHeader>,
    ) -> UDPAsyncWorker {
        let rx = receiver.clone();
        let server_port = format!("{}:{}", arguments.server, arguments.port);
        let source_ip_addr = format!("{}:0", arguments.source);
        let edns_size_local = arguments.edns_size as usize;
        let check_all_message = arguments.check_all_message;
        let result_sender_local = result_sender.clone();

        let thread = std::thread::spawn(move || {
            let rt = runtime::Builder::new_multi_thread().enable_all().build().unwrap();
            let handler = rt.spawn(async move {
                // let tx = tx.clone();
                let socket = UdpSocket::bind(source_ip_addr).await.expect("unable to bind to server");
                socket
                    .connect(server_port).await
                    .expect("unable to connect to server");
                loop {
                    let data = match rx.lock().unwrap().recv() {
                        Ok(data) => data,
                        Err(_) => {
                            break;
                        }
                    };
                    debug!("send {:?}", data.as_slice());
                    let start = Instant::now();
                    if let Err(e) = socket.send(data.as_slice()).await {
                        error!("send error : {}", e);
                    };

                    if check_all_message == true {
                        let mut buffer = vec![0; edns_size_local];
                        if let Ok(size) = socket.recv(&mut buffer).await {
                            if let Ok(message) = Message::from_bytes(&buffer[..size]) {
                                if let Err(e) = result_sender_local.send(MessageOrHeader::Message((
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
                        if let Ok(size) = socket.recv(&mut buffer).await {
                            if let Ok(message) = Header::from_bytes(&buffer[..size]) {
                                if let Err(e) = result_sender_local.send(MessageOrHeader::Header((
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
                };
            });
            rt.block_on(async {
                handler.await;
            });
            debug!("udp worker thread exit success");
            drop(result_sender);
        });
        UDPAsyncWorker {
            write_thread: Some(thread),
        }
    }
}
