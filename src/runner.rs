use crate::arguments::{Argument, Protocol};
use crate::cache::Cache;
use crossbeam_channel::{bounded, Receiver, Sender};

use crate::report::{QueryStatusStore, ReportType, RunnerReport};
use governor::{Quota, RateLimiter};
use nonzero_ext::*;
use std::io::{Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use trust_dns_client::op::Message;
use trust_dns_client::proto::serialize::binary::BinDecodable;

pub struct Runner {
    arguments: Argument,
    workers: Vec<Box<dyn Worker>>,
    producer: QueryProducer,
    consumer: QueryConsumer,
    report: RunnerReport,
}

impl Runner {
    pub(crate) fn new(arguments: Argument) -> Runner {
        let mut workers: Vec<Box<dyn Worker>> = Vec::new();
        // let (sender, receiver) = channel();
        let (sender, receiver) = bounded(arguments.client);
        let receiver = Arc::new(Mutex::new(receiver));
        let origin_arguments = Arc::new(arguments.clone());

        let producer = QueryProducer::new(arguments.clone(), sender.clone());

        let (result_sender, result_receiver) = bounded(arguments.client);
        for i in 0..origin_arguments.client {
            let result_sender = result_sender.clone();
            match arguments.protocol {
                Protocol::UDP => {
                    workers.push(Box::new(UDPWorker::new(
                        i,
                        arguments.clone(),
                        receiver.clone(),
                        result_sender,
                    )));
                }
                Protocol::TCP => {
                    workers.push(Box::new(TCPWorker::new(
                        i,
                        arguments.clone(),
                        receiver.clone(),
                        result_sender,
                    )));
                }
            }
        }

        let consumer = QueryConsumer::new(arguments.clone(), result_receiver);
        Runner {
            arguments,
            workers,
            producer,
            consumer,
            report: RunnerReport::new(),
        }
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        for worker in &mut self.workers {
            worker.join();
            // if let Some(handler) = worker.write_thread.take() {
            //     handler.join().expect("fail to join thread");
            // }
        }
        self.report
            .set_producer_report((*self.producer.store.lock().unwrap()).clone());
        self.report
            .set_consumer_report((*self.consumer.store.lock().unwrap()).clone());
        self.report.report(ReportType::Basic);
    }
}

struct QueryProducer {
    store: Arc<Mutex<QueryStatusStore>>,
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
                Some(RateLimiter::direct(
                    Quota::per_second(
                        NonZeroU32::new(argument.qps as u32).expect("qps setting error"),
                    )
                    .allow_burst(NonZeroU32::new(1).unwrap()),
                ))
            }
        };
        let mut current_counter: usize = 0;
        let mut cache = Cache::new(&argument.clone());
        let store = Arc::new(Mutex::new(QueryStatusStore::default()));
        let thread_store = store.clone();
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
                        current_counter = current_counter + 1;
                        let (data, query_type) = cache.build_message();
                        sender.send(data);
                        if let Ok(mut v) = thread_store.lock() {
                            v.update_query(query_type);
                        }
                    } else {
                        break;
                    }
                }
            }
            debug!("producer thread quit");
            drop(sender);
        });
        QueryProducer { store, thread }
    }
}

trait Worker {
    fn join(&mut self);
}

struct UDPWorker {
    id: usize,
    arguments: Argument,
    receiver: Arc<Mutex<Receiver<Vec<u8>>>>,
    write_thread: Option<std::thread::JoinHandle<()>>,
    read_thread: Option<std::thread::JoinHandle<()>>,
}
impl Worker for UDPWorker {
    fn join(&mut self) {
        if let Some(handler) = self.write_thread.take() {
            handler.join().expect("fail to join thread");
        }
    }
}

impl UDPWorker {
    fn new(
        id: usize,
        arguments: Argument,
        receiver: Arc<Mutex<Receiver<Vec<u8>>>>,
        result_sender: Sender<Message>,
    ) -> UDPWorker {
        let rx = receiver.clone();
        let server_port = format!("{}:{}", arguments.server, arguments.port);
        let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
        socket
            .connect(server_port)
            .expect("unable to connect to server");
        socket.set_nonblocking(true).expect("set udp unblock fail");
        let timeout = arguments.timeout as u64;
        let thread = std::thread::spawn(move || {
            loop {
                match rx.lock().unwrap().recv() {
                    Ok(data) => {
                        debug!("send {:?}", data.as_slice());
                        match socket.send(data.as_slice()) {
                            Err(e) => {
                                println!("send error : {}", e);
                            }
                            Ok(_) => {}
                        };
                    }
                    Err(_e) => {
                        break;
                    }
                };
                let mut buffer = [0u8; 1234];
                match socket.recv(&mut buffer) {
                    Ok(bit_received) => {
                        debug!("receive {:?}", &buffer[..bit_received]);
                        if let Ok(message) = Message::from_bytes(&buffer[..bit_received]) {
                            result_sender.send(message);
                        }
                    }
                    _ => {}
                }
            }
            let wait_start = std::time::Instant::now();
            loop {
                // wait for more seconds and return
                if wait_start.elapsed() > std::time::Duration::from_secs(timeout) {
                    break;
                }
                let mut buffer = [0u8; 1234];
                match socket.recv(&mut buffer) {
                    Ok(bit_received) => {
                        debug!("receive {:?}", &buffer[..bit_received]);
                        if let Ok(message) = Message::from_bytes(&buffer[..bit_received]) {
                            result_sender.send(message);
                        }
                    }
                    _ => {}
                }
            }
            debug!("udp worker thread {} exit success", id);
            drop(result_sender);
        });
        UDPWorker {
            id,
            arguments,
            receiver,
            write_thread: Some(thread),
            read_thread: None,
        }
    }
}

struct TCPWorker {
    id: usize,
    arguments: Argument,
    receiver: Arc<Mutex<Receiver<Vec<u8>>>>,
    write_thread: Option<std::thread::JoinHandle<()>>,
    read_thread: Option<std::thread::JoinHandle<()>>,
}

impl Worker for TCPWorker {
    fn join(&mut self) {
        if let Some(handler) = self.write_thread.take() {
            handler.join().expect("fail to join thread");
        }
    }
}

impl TCPWorker {
    fn new(
        id: usize,
        arguments: Argument,
        receiver: Arc<Mutex<Receiver<Vec<u8>>>>,
        result_sender: Sender<Message>,
    ) -> TCPWorker {
        let rx = receiver.clone();
        let server_port = format!("{}:{}", arguments.server, arguments.port);

        // stream.set_nonblocking(true).expect("set tcp unblock fail");
        let mut stream = TcpStream::connect(server_port.clone())
            .expect(format!("unable to connect to server :{}", server_port).as_str());
        stream.set_read_timeout(Some(std::time::Duration::from_secs(
            arguments.timeout as u64,
        )));
        let thread = std::thread::spawn(move || {
            loop {
                match rx.lock().unwrap().recv() {
                    Ok(data) => {
                        debug!("send {:?}", data.as_slice());
                        match stream.write(data.as_slice()) {
                            Err(e) => {
                                println!("send error : {}", e);
                            }
                            Ok(_) => {}
                        };
                        let mut buffer = [0u8; 1234];
                        match stream.read(&mut buffer) {
                            Ok(bit_received) => {
                                debug!("receive {:?}", &buffer[..bit_received]);
                                // tcp data with two bite length
                                if let Ok(message) = Message::from_bytes(&buffer[2..bit_received]) {
                                    result_sender.send(message);
                                }
                            }
                            Err(err) => println!("receive error: {}", err),
                        }
                    }
                    Err(_e) => {
                        break;
                    }
                };
            }
            debug!("tcp worker thread {} exit success", id);
            drop(result_sender);
        });
        TCPWorker {
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
    end: std::time::Instant,
    receiver: Receiver<Message>,
    thread: Option<JoinHandle<()>>,
    store: Arc<Mutex<QueryStatusStore>>,
}

impl QueryConsumer {
    fn new(arguments: Argument, receiver: Receiver<Message>) -> QueryConsumer {
        let thread_receiver = receiver.clone();
        let store = Arc::new(Mutex::new(QueryStatusStore::default()));
        let thread_store = store.clone();
        let thread = std::thread::spawn(move || {
            for _message in thread_receiver {
                match thread_store.lock() {
                    Ok(mut v) => {
                        v.update_response(&_message);
                    }
                    _ => {}
                }
            }
        });

        QueryConsumer {
            arguments,
            end: std::time::Instant::now(),
            receiver,
            thread: Some(thread),
            store,
        }
    }
}
