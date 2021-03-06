use crossbeam_channel::{Receiver, Sender};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use trust_dns_client::op::{Header, Message};
use trust_dns_client::proto::serialize::binary::BinDecodable;

use super::{MessageOrHeader, Worker, HEADER_SIZE};
use crate::arguments::Argument;

pub struct DoTWorker {
    write_thread: Option<std::thread::JoinHandle<()>>,
}

impl Worker for DoTWorker {
    fn run(&mut self) {
        if let Some(handler) = self.write_thread.take() {
            handler.join().expect("fail to join dot thread");
        }
    }
}

impl DoTWorker {
    pub fn new(
        arguments: Argument,
        receiver: Arc<Mutex<Receiver<Vec<u8>>>>,
        result_sender: Sender<MessageOrHeader>,
    ) -> DoTWorker {
        let rx = receiver.clone();
        let server_port = format!("{}:{}", arguments.server, arguments.port);
        let check_all_message = arguments.check_all_message;

        // stream.set_nonblocking(true).expect("set tcp unblock fail");
        let thread = std::thread::spawn(move || {
            let mut socket = TcpStream::connect(server_port.clone())
                .expect(format!("unable to connect to server :{}", server_port).as_str());
            if let Err(e) = socket.set_read_timeout(Some(std::time::Duration::from_secs(
                arguments.timeout as u64,
            ))) {
                error!("read_timeout {:?}", e);
            }
            let mut config = rustls::ClientConfig::new();
            config
                .root_store
                .add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);

            let arc = std::sync::Arc::new(config);
            let dns_name =
                webpki::DNSNameRef::try_from_ascii_str(arguments.server.as_str()).unwrap();
            let mut client = rustls::ClientSession::new(&arc, dns_name);
            let mut stream = rustls::Stream::new(&mut client, &mut socket);

            loop {
                let data = match rx.lock().unwrap().recv() {
                    Ok(data) => data,
                    Err(_) => {
                        break;
                    }
                };
                debug!("send {:?}", data.as_slice());
                let start = Instant::now();
                match stream.write(data.as_slice()) {
                    Err(e) => {
                        println!("send error : {}", e);
                    }
                    Ok(_) => {}
                };
                let mut length = vec![0u8; 2];
                if let Err(_e) = stream.read_exact(&mut length) {
                    continue;
                };
                let size = (length[0] as usize) << 8 | length[1] as usize;
                let mut data = vec![0; size];
                if let Err(e) = stream.read_exact(&mut data) {
                    debug!("receive error: {}", e.to_string())
                }
                if check_all_message == true {
                    if let Ok(message) = Message::from_bytes(data.as_slice()) {
                        if let Err(e) = result_sender.send(MessageOrHeader::Message((
                            message,
                            start.elapsed().as_secs_f64(),
                        ))) {
                            error!("send packet: {}", e.to_string())
                        };
                    }
                } else {
                    if let Ok(message) = Header::from_bytes(&data[0..HEADER_SIZE]) {
                        let code = message.response_code();
                        if code != trust_dns_client::proto::op::ResponseCode::NoError.low() {
                            println!("{} --- {:?}", size, message)
                        }
                        if let Err(e) = result_sender.send(MessageOrHeader::Header((
                            message,
                            start.elapsed().as_secs_f64(),
                        ))) {
                            error!("send packet: {}", e.to_string())
                        };
                    }
                }
            }
            debug!("tcp worker thread exit success");
            drop(result_sender);
        });
        DoTWorker {
            write_thread: Some(thread),
        }
    }
}
