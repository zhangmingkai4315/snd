use crossbeam_channel::{Receiver, Sender};
use std::io::{Read, Write};
use std::net::{TcpStream};
use std::sync::{Arc, Mutex};
use trust_dns_client::op::Message;
use trust_dns_client::proto::serialize::binary::BinDecodable;


use super::Worker;
use crate::arguments::Argument;

pub struct TCPWorker {
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
    pub fn new(
        arguments: Argument,
        receiver: Arc<Mutex<Receiver<Vec<u8>>>>,
        result_sender: Sender<Message>,
    ) -> TCPWorker {
        let rx = receiver.clone();
        let server_port = format!("{}:{}", arguments.server, arguments.port);
        // stream.set_nonblocking(true).expect("set tcp unblock fail");
        let thread = std::thread::spawn(move || {
            // TODO : tcp source ip setting
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
