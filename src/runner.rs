use crate::arguments::Argument;
use crate::cache::Cache;
use crossbeam_channel::{bounded, Receiver, Sender};
use futures::future::join_all;
use governor::{Quota, RateLimiter};
use nonzero_ext::*;
use std::net::{SocketAddr, UdpSocket};
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use trust_dns_client::op::Message;
use trust_dns_client::proto::serialize::binary::BinDecodable;
use trust_dns_client::udp::UdpClientConnection;

pub struct Runner {
    arguments: Argument,
    workers: Vec<QueryWorker>,
    producer: QueryProducer,
    consumer: QueryConsumer,
}

impl Runner {
    pub(crate) fn new(arguments: Argument) -> Runner {
        let mut workers = Vec::new();
        // let (sender, receiver) = channel();
        let (sender, receiver) = bounded(arguments.client);
        let receiver = Arc::new(Mutex::new(receiver));
        let origin_arguments = Arc::new(arguments.clone());

        let producer = QueryProducer::new(arguments.clone(), sender.clone());

        let (resultSender, resultReceiver) = bounded(arguments.client);
        for i in 0..origin_arguments.client {
            let resultSender = resultSender.clone();
            workers.push(QueryWorker::new(
                i,
                arguments.clone(),
                receiver.clone(),
                resultSender,
            ));
        }

        let consumer = QueryConsumer::new(arguments.clone(), resultReceiver);
        Runner {
            arguments,
            workers,
            producer,
            consumer,
        }
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        for worker in &mut self.workers {
            if let Some(handler) = worker.write_thread.take() {
                handler.join().expect("fail to join thread");
            }
        }
        self.consumer.report();
    }
}

struct QueryProducer {
    thread: std::thread::JoinHandle<()>,
}

impl QueryProducer {
    fn new(argument: Argument, sender: Sender<Vec<u8>>) -> QueryProducer {
        let sender = sender.clone();
        let max_counter = argument.max;
        let qps = argument.qps;
        let rate_limiter = {
            if qps == 0 {
                None
            } else {
                Some(RateLimiter::direct(Quota::per_second(
                    NonZeroU32::new(argument.qps as u32).expect("qps setting error"),
                )))
            }
        };
        let mut current_counter: usize = 0;
        let mut cache = Cache::new(&argument.clone());
        let thread = std::thread::spawn(move || {
            let limiter = rate_limiter.as_ref();
            loop {
                let mut ready = false;
                match limiter {
                    Some(v) => {
                        if v.check().is_ok() == true {
                            ready = true;
                        }
                    }
                    _ => ready = true,
                }
                if ready == true {
                    if current_counter != max_counter {
                        println!("{}", current_counter);
                        current_counter = current_counter + 1;
                        sender.send(cache.build_message());
                    } else {
                        break;
                    }
                }
            }
            info!("producer thread quit");
            drop(sender);
        });
        QueryProducer { thread }
    }
}

struct QueryWorker {
    id: usize,
    arguments: Argument,
    receiver: Arc<Mutex<Receiver<Vec<u8>>>>,
    write_thread: Option<std::thread::JoinHandle<()>>,
    read_thread: Option<std::thread::JoinHandle<()>>,
}
impl QueryWorker {
    fn new(
        id: usize,
        arguments: Argument,
        receiver: Arc<Mutex<Receiver<Vec<u8>>>>,
        resultSender: Sender<Message>,
    ) -> QueryWorker {
        let rx = receiver.clone();
        let server_port = format!("{}:{}", arguments.server, arguments.port);
        let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
        socket
            .connect(server_port)
            .expect("unable to connect to server");
        socket.set_nonblocking(true).expect("set udp unblock fail");
        let thread = std::thread::spawn(move || {
            loop {
                match rx.lock().unwrap().recv() {
                    Ok(data) => {
                        println!("send {:?}", data.as_slice());
                        match socket.send(data.as_slice()) {
                            Err(e) => {
                                println!("send error : {}", e);
                            }
                            Ok(_) => {}
                        };
                    }
                    Err(e) => {
                        break;
                    }
                };
                let mut buffer = [0u8; 1234];
                match socket.recv(&mut buffer) {
                    Ok(bit_received) => {
                        if let Ok(message) = Message::from_bytes(&buffer[..bit_received]) {
                            resultSender.send(message);
                        }
                    }
                    _ => {}
                }
            }
            info!("worker thread {} exit success", id);
            drop(resultSender);
        });
        QueryWorker {
            id,
            arguments,
            receiver,
            write_thread: Some(thread),
            read_thread: None,
        }
    }
}

struct QueryConsumer {
    arguments: Argument,
    receiver: Receiver<Message>,
    thread: Option<JoinHandle<()>>,
}

impl QueryConsumer {
    fn new(arguments: Argument, receiver: Receiver<Message>) -> QueryConsumer {
        let thread_receiver = receiver.clone();
        let thread = std::thread::spawn(move || for message in thread_receiver {});

        QueryConsumer {
            arguments,
            receiver,
            thread: Some(thread),
        }
    }

    fn report(&self) {
        println!("consumer print report...");
    }
}
