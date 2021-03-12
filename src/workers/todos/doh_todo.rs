use super::{MessageOrHeader, Worker, HEADER_SIZE};
use crate::runner::consumer::ResponseConsumer;
use crate::runner::producer::PacketGeneratorStatus;
use crate::runner::report::StatusStore;
use crate::runner::QueryProducer;
use crate::utils::{Argument, DoHMethod};
use base64;
use bytes::Bytes;
use crossbeam_channel::Sender;
use h2::client::{self, Connection, SendRequest};
use http::Request;
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::ops::Add;
use tokio::net::TcpStream;
use trust_dns_client::op::Header;
use trust_dns_client::proto::serialize::binary::BinDecodable;
use std::sync::{Arc,Mutex};
use reqwest::blocking::Client;
pub struct DOHWorker {
    arguments: Argument,
}

impl DOHWorker {
    pub fn new(arguments: Argument) -> Box<dyn Worker> {
        Box::new(DOHWorker { arguments })
    }
}

impl Worker for DOHWorker {
    fn run(
        &mut self,
        id: usize,
        sender: Sender<(StatusStore, StatusStore)>,
    ) -> (StatusStore, StatusStore) {
        let arguments = self.arguments.clone();
        let interval = self.arguments.output_interval as u64;
        let mut next_status_send =
            std::time::SystemTime::now().add(std::time::Duration::from_secs(interval));
        let mut stop_sender_timer = std::time::SystemTime::now();
        let max_send = self.arguments.max as u64;
        let mut send_counter: u64 = 0;
        let mut receive_counter: u64 = 0;
        let start = std::time::SystemTime::now();
        let mut time_store = HashMap::new();
        let mut producer = QueryProducer::new(arguments.clone());
        let mut consumer = ResponseConsumer::new();

        let url = arguments.doh_server.clone();
        let client = Client::new();
        'outer: loop {
            match producer.retrieve() {
                PacketGeneratorStatus::Success(data, qtype) => {
                    let key = ((data[0] as u16) << 8) | (data[1] as u16);
                    time_store.insert(key, chrono::Utc::now().timestamp_nanos());
                    let res = match arguments.doh_server_method {
                        DoHMethod::Post => client
                            .post(&url)
                            .header("accept", "application/dns-message")
                            .header("content-type", "application/dns-message")
                            .body(data),
                        DoHMethod::Get => {
                            let base64_data = base64::encode(data);
                            client
                                .get(&format!("{}?dns={}", &url, base64_data))
                                .header("accept", "application/dns-message")
                                .header("content-type", "application/dns-message")
                        }
                    };

                    if let Ok(resp) = res.send() {
                        if let Ok(buffer) = resp.bytes() {
                            let duration = match time_store.get(&key) {
                                Some(start) => {
                                    (chrono::Utc::now().timestamp_nanos() - start) as f64
                                        / 1000000000.0
                                }
                                _ => 0.0,
                            };
                            if let Ok(message) = Header::from_bytes(&buffer[0..HEADER_SIZE]) {
                                consumer
                                    .receive(&MessageOrHeader::Header((message, duration)));
                                receive_counter += 1;
                                send_counter += 1;
                                // producer.store.update_query(qtype);
                                stop_sender_timer = std::time::SystemTime::now();
                            }
                        }
                    }
                }
                _ => {}
            }
            // if (max_send > 0 && (receive_counter == max_send))
            //     || stop_sender_timer.elapsed().unwrap() > std::time::Duration::from_secs(5)
            // {
            //     debug!(
            //         "should break loop send = {} receive = {} cpu={}",
            //         send_counter, receive_counter, id
            //     );
            //     break ;
            // }
            // if interval != 0 {
            //     let now = std::time::SystemTime::now();
            //     if now >= next_status_send {
            //         producer
            //             .store
            //             .set_send_duration(now.duration_since(start).unwrap());
            //         consumer.store.set_receive_total(receive_counter);
            //         consumer.update_report();
            //         if let Err(err) = sender.send((producer.store.clone(), consumer.store.clone()))
            //         {
            //             error!("send interval status fail: {:?}", err)
            //         }
            //         next_status_send = now.add(std::time::Duration::from_secs(interval));
            //     }
            // }
        }

        std::mem::drop(sender);
        producer
            .store
            .set_send_duration(stop_sender_timer.duration_since(start).unwrap());
        consumer.store.set_receive_total(receive_counter);
        consumer.receive(&MessageOrHeader::End);
        (producer.store, consumer.store)
    }
}
