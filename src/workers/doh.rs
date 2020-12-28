use base64;
use crossbeam_channel::{Receiver, Sender};
use reqwest::blocking::Client;
use std::sync::{Arc, Mutex};
use trust_dns_client::op::{Header, Message};
use trust_dns_client::proto::serialize::binary::BinDecodable;

use super::{MessageOrHeader, Worker, HEADER_SIZE};
use crate::arguments::{Argument, DoHMethod};

pub struct DOHWorker {
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
    pub fn new(
        arguments: Argument,
        receiver: Arc<Mutex<Receiver<Vec<u8>>>>,
        result_sender: Sender<MessageOrHeader>,
    ) -> DOHWorker {
        let rx = receiver.clone();
        let url = arguments.doh_server.clone();
        let client = Client::new();
        let check_all_message = arguments.check_all_message;

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

                if let Ok(resp) = res.send() {
                    if let Ok(buffer) = resp.bytes() {
                        if check_all_message == true {
                            if let Ok(message) = Message::from_bytes(&buffer) {
                                if let Err(e) =
                                    result_sender.send(MessageOrHeader::Message(message))
                                {
                                    error!("send packet: {}", e)
                                };
                            }
                        } else {
                            if let Ok(message) = Header::from_bytes(&buffer[0..HEADER_SIZE]) {
                                if let Err(e) = result_sender.send(MessageOrHeader::Header(message))
                                {
                                    error!("send packet: {}", e)
                                };
                            }
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
