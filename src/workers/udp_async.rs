use super::{MessageOrHeader, Worker, HEADER_SIZE};
use crate::arguments::Argument;
use crossbeam_channel::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::net::UdpSocket;
use tokio::runtime;
use trust_dns_client::op::{Header, Message};
use trust_dns_client::proto::serialize::binary::BinDecodable;

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
        let mut thread_handlers = vec![];
        let thread = std::thread::spawn(move || {
            let rt = runtime::Builder::new_multi_thread()
                .worker_threads(arguments.client)
                .thread_name_fn(|| {
                    static ATOMIC_ID: std::sync::atomic::AtomicI32 =
                        std::sync::atomic::AtomicI32::new(0);
                    let id = ATOMIC_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    format!("async-thread-{}", id)
                })
                .enable_all()
                .build()
                .unwrap();

            for _i in 1..=arguments.client {
                let rx = rx.clone();
                let result_send_local = result_sender.clone();
                let source_ip_addr = source_ip_addr.clone();
                let server_port = server_port.clone();
                let task = async move {
                    debug!(
                        "before bind in thread : {:?}",
                        std::thread::current().name()
                    );
                    let socket = UdpSocket::bind(source_ip_addr)
                        .await
                        .expect("unable to bind to server");
                    socket
                        .connect(server_port)
                        .await
                        .expect("unable to connect to server");
                    loop {
                        let data = match rx.lock().unwrap().recv() {
                            Ok(data) => data,
                            Err(_) => {
                                break;
                            }
                        };

                        let start = Instant::now();
                        debug!(
                            "before send in thread : {:?}",
                            std::thread::current().name()
                        );

                        if let Ok(message) = Header::from_bytes(&data[..HEADER_SIZE]) {
                            if let Err(e) = result_send_local.send(MessageOrHeader::Header((
                                message,
                                start.elapsed().as_secs_f64(),
                            ))) {
                                error!("send packet: {:?}", e);
                            };
                        } else {
                            error!("parse dns message error");
                        }
                    }
                    //     if let Err(e) = socket.send(data.as_slice()).await {
                    //         error!("send error : {}", e);
                    //     };
                    //     if check_all_message == true {
                    //         let mut buffer = vec![0; edns_size_local];
                    //         if let Ok(size) = socket.recv(&mut buffer).await {
                    //             if let Ok(message) = Message::from_bytes(&buffer[..size]) {
                    //                 if let Err(e) =
                    //                 result_send_local.send(MessageOrHeader::Message((
                    //                     message,
                    //                     start.elapsed().as_secs_f64(),
                    //                 )))
                    //                 {
                    //                     error!("send packet: {:?}", e);
                    //                 };
                    //             } else {
                    //                 error!("parse dns message error");
                    //             }
                    //         }
                    //     } else {
                    //         let mut buffer = vec![0; HEADER_SIZE];
                    //         debug!(
                    //             "before recv in thread : {:?}",
                    //             std::thread::current().name()
                    //         );
                    //         if let Ok(size) = socket.recv(&mut buffer).await {
                    //             if let Ok(message) = Header::from_bytes(&buffer[..size]) {
                    //                 if let Err(e) = result_send_local.send(MessageOrHeader::Header(
                    //                     (message, start.elapsed().as_secs_f64()),
                    //                 )) {
                    //                     error!("send packet: {:?}", e);
                    //                 };
                    //                 debug!(
                    //                     "send to channel in thread : {:?}",
                    //                     std::thread::current().name()
                    //                 );
                    //             } else {
                    //                 error!("parse dns message error");
                    //             }
                    //         }
                    //     }
                    // }
                };
                thread_handlers.push(rt.spawn(task));
            }

            rt.block_on(async {
                for i in thread_handlers {
                    i.await;
                }
            });
            debug!("udp worker thread exit success");
            drop(result_sender);
        });
        UDPAsyncWorker {
            write_thread: Some(thread),
        }
    }
}
