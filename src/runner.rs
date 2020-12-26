use crate::arguments::{Argument, DoHMethod, Protocol};
use crate::cache::Cache;
use crate::report::{QueryStatusStore, ReportType, RunnerReport};
use base64;
use crossbeam_channel::{bounded, Receiver, Sender};
use governor::{Quota, RateLimiter};
// use net2::TcpBuilder;
use reqwest::blocking::Client;
use std::io::{Read, Write};
// use net2::TcpStreamExt;
// use net2::ext::TcpStreamExt;
use std::net::{TcpStream, UdpSocket};
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use trust_dns_client::op::Message;
use trust_dns_client::proto::serialize::binary::BinDecodable;
// use trust_dns_client::proto::tcp::TcpClientConnect;

pub struct Runner {
    // arguments: Argument,
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
        for _i in 0..origin_arguments.client {
            let result_sender = result_sender.clone();
            match arguments.protocol {
                Protocol::UDP => {
                    workers.push(Box::new(UDPWorker::new(
                        arguments.clone(),
                        receiver.clone(),
                        result_sender,
                    )));
                }
                Protocol::TCP => {
                    workers.push(Box::new(TCPWorker::new(
                        arguments.clone(),
                        receiver.clone(),
                        result_sender,
                    )));
                }
                Protocol::DOH => workers.push(Box::new(DOHWorker::new(
                    arguments.clone(),
                    receiver.clone(),
                    result_sender,
                ))),
            }
        }

        let consumer = QueryConsumer::new(result_receiver);
        Runner {
            // arguments,
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
            worker.block();
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
        std::thread::spawn(move || {
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
                        if let Err(e) = sender.send(data) {
                            error!("send packet: {}", e);
                        }
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
        QueryProducer {
            store,
            // thread
        }
    }
}

trait Worker {
    fn block(&mut self) {}
}

struct UDPWorker {
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
    fn new(
        arguments: Argument,
        receiver: Arc<Mutex<Receiver<Vec<u8>>>>,
        result_sender: Sender<Message>,
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
        let thread = std::thread::spawn(move || {
            loop {
                // TODO: each thread has own producer?
                let data = match rx.lock().unwrap().recv() {
                    Ok(data) => data,
                    Err(_) => {
                        break;
                    }
                };
                debug!("send {:?}", data.as_slice());
                if let Err(e) = socket.send(data.as_slice()) {
                    error!("send error : {}", e);
                };
                let mut buffer = vec![0; edns_size_local];
                if let Ok(size) = socket.recv(&mut buffer) {
                    if let Ok(message) = Message::from_bytes(&buffer[..size]) {
                        if let Err(e) = result_sender.send(message) {
                            error!("send packet: {:?}", e);
                        };
                    } else {
                        error!("parse dns message error");
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

struct TCPWorker {
    write_thread: Option<std::thread::JoinHandle<()>>,
}

impl Worker for TCPWorker {
    fn block(&mut self) {
        if let Some(handler) = self.write_thread.take() {
            handler.join().expect("fail to join doh thread");
        }
    }
}

impl TCPWorker {
    fn new(
        arguments: Argument,
        receiver: Arc<Mutex<Receiver<Vec<u8>>>>,
        result_sender: Sender<Message>,
    ) -> TCPWorker {
        let rx = receiver.clone();
        let server_port = format!("{}:{}", arguments.server, arguments.port);
        // stream.set_nonblocking(true).expect("set tcp unblock fail");
        let thread = std::thread::spawn(move || {
            // TcpStream::fr
            // let builder = TcpBuilder::new_v4().expect("unable using tcp v4 resource");
            // let source_ip_addr = format!("{}:0", arguments.source);
            // let sock = TcpListener::bind(source_ip_addr).unwrap();
            let mut stream = TcpStream::connect(server_port.clone())
                .expect(format!("unable to connect to server :{}", server_port).as_str());

            if let Err(e) = stream.set_read_timeout(Some(std::time::Duration::from_secs(
                arguments.timeout as u64,
            ))) {
                error!("read_timeout {:?}", e);
            }
            // if let Err(e) = stream.set_keepalive(Some(std::time::Duration::from_secs(60))){
            //     error!("{}", e.to_string());
            //     return
            // }
            loop {
                let data = match rx.lock().unwrap().recv() {
                    Ok(data) => data,
                    Err(_) => {
                        break;
                    }
                };
                debug!("send {:?}", data.as_slice());
                match stream.write(data.as_slice()) {
                    Err(e) => {
                        println!("send error : {}", e);
                    }
                    Ok(_) => {}
                };
                let mut buffer = vec![0; 1500];
                match stream.read(&mut buffer) {
                    Ok(bit_received) => {
                        debug!("receive {:?}", &buffer[..bit_received]);
                        if bit_received <= 2 {
                            continue;
                        }
                        // tcp data with two bite length
                        if let Ok(message) = Message::from_bytes(&buffer[2..bit_received]) {
                            if let Err(e) = result_sender.send(message) {
                                error!("send packet: {}", e)
                            };
                        }
                    }
                    Err(err) => println!("receive error: {}", err),
                };
            }
            debug!("tcp worker thread exit success");
            drop(result_sender);
        });
        TCPWorker {
            write_thread: Some(thread),
        }
    }
}

struct DOHWorker {
    write_thread: Option<std::thread::JoinHandle<()>>,
}

impl Worker for DOHWorker {
    fn block(&mut self) {
        if let Some(handler) = self.write_thread.take() {
            handler.join().expect("fail to join doh thread");
        }
    }
}

impl DOHWorker {
    fn new(
        arguments: Argument,
        receiver: Arc<Mutex<Receiver<Vec<u8>>>>,
        result_sender: Sender<Message>,
    ) -> DOHWorker {
        let rx = receiver.clone();
        let url = arguments.doh_server.clone();
        let client = Client::new();
        let thread = std::thread::spawn(move || {
            loop {
                let data = match rx.lock().unwrap().recv() {
                    Ok(data) => data,
                    Err(_) => {
                        break;
                    }
                };
                debug!("send {:?}", data.as_slice());

                // let length = data.len();
                let res = match arguments.doh_server_method {
                    DoHMethod::Post => client
                        .post(&url)
                        .header("accept", "application/dns-message")
                        .header("content-type", "application/dns-message")
                        .body(data),
                    DoHMethod::Get => {
                        let base64_data = base64::encode(data.as_slice());
                        client
                            .get(&format!("{}?dns={}", &url, base64_data))
                            .header("accept", "application/dns-message")
                            .header("content-type", "application/dns-message")
                    }
                };
                println!("{:?}", res);
                if let Ok(resp) = res.send() {
                    if let Ok(buffer) = resp.bytes() {
                        if let Ok(message) = Message::from_bytes(&buffer) {
                            if let Err(e) = result_sender.send(message) {
                                error!("send packet: {}", e)
                            };
                        }
                    }
                };
            }
            debug!("tcp worker thread exit success");
            drop(result_sender);
        });
        DOHWorker {
            write_thread: Some(thread),
        }
    }
}

struct QueryConsumer {
    store: Arc<Mutex<QueryStatusStore>>,
}

impl QueryConsumer {
    fn new(receiver: Receiver<Message>) -> QueryConsumer {
        let thread_receiver = receiver.clone();
        let store = Arc::new(Mutex::new(QueryStatusStore::default()));
        let thread_store = store.clone();
        std::thread::spawn(move || {
            for _message in thread_receiver {
                match thread_store.lock() {
                    Ok(mut v) => {
                        v.update_response(&_message);
                    }
                    _ => {}
                }
            }
        });

        QueryConsumer { store }
    }
}
