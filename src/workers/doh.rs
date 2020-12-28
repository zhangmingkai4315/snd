use base64;
use crossbeam_channel::{Receiver, Sender};
use reqwest::blocking::Client;
use std::sync::{Arc, Mutex};
use trust_dns_client::op::Message;
use trust_dns_client::proto::serialize::binary::BinDecodable;

use super::Worker;
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
