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

pub struct DOHWorker {
    arguments: Argument,
    sockets: Vec<(SendRequest<Bytes>, Connection<TcpStream>)>,
}

impl DOHWorker {
    pub fn new(arguments: Argument) -> Box<dyn Worker> {
        let mut sockets = vec![];
        let server = arguments.clone().doh_server;
        let tokio_runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        // block until all client connect success
        tokio_runtime.block_on(async {
            for _i in 0..arguments.clone().client {
                let stream = TcpStream::connect(server.clone()).await;
                if let Err(e) = stream {
                    error!("{}", e.to_string());
                    continue;
                }
                let stream = client::handshake(stream.unwrap()).await;
                if let Err(e) = stream {
                    error!("{}", e.to_string());
                    continue;
                }

                sockets.push(stream.unwrap());
            }
        });
        Box::new(DOHWorker { arguments, sockets })
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

        #[allow(unused_assignments)]
        let mut stop_sender_timer = std::time::SystemTime::now();
        let max_send = self.arguments.max as u64;
        let mut send_counter: u64 = 0;
        let mut receive_counter: u64 = 0;
        let start = std::time::SystemTime::now();
        let mut time_store = HashMap::new();
        let server_method = &self.arguments.doh_server_method;
        let tokio_runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let server = self.arguments.doh_server.as_str();
        tokio_runtime.block_on(async move {
            let counter: usize = 0;
            let socket_size = self.sockets.len();
            let mut producer = QueryProducer::new(arguments.clone());
            let mut consumer = ResponseConsumer::new();

            'outer: loop {
                let index = counter % socket_size;
                let current_socket = self.sockets.get_mut(index).unwrap();
                match producer.retrieve() {
                    PacketGeneratorStatus::Success(data, qtype) => {
                        let key = ((data[0] as u16) << 8) | (data[1] as u16);
                        time_store.insert(key, chrono::Utc::now().timestamp_nanos());
                        let query = match server_method.clone() {
                            DoHMethod::Post => {
                                let request = Request::post(server)
                                    .header("accept", "application/dns-message")
                                    .header("content-type", "application/dns-message")
                                    .body(())
                                    .unwrap();
                                let (response, mut stream) =
                                    current_socket.0.send_request(request, false).unwrap();
                                stream.send_data(Bytes::from(data), false);
                                (response, stream)
                            }
                            DoHMethod::Get => {
                                let base64_data = base64::encode(data);
                                let request =
                                    Request::get(&format!("{}?dns={}", &server, base64_data))
                                        .header("accept", "application/dns-message")
                                        .header("content-type", "application/dns-message")
                                        .body(())
                                        .unwrap();
                                let (response, stream) =
                                    current_socket.0.send_request(request, false).unwrap();
                                (response, stream)
                            }
                        };
                        match query.0.await {
                            Ok(response) => {
                                let mut body = response.into_body();
                                if let Some(buffer) = body.data().await {
                                    if buffer.is_err() {
                                        error!("{}", buffer.unwrap_err().to_string());
                                        continue 'outer;
                                    }
                                    let dns_packet = buffer.unwrap().as_ref();
                                    let key =
                                        ((dns_packet[2] as u16) << 8) | (dns_packet[3] as u16);
                                    let duration = match time_store.get(&key) {
                                        Some(start) => {
                                            (chrono::Utc::now().timestamp_nanos() - start) as f64
                                                / 1000000000.0
                                        }
                                        _ => 0.0,
                                    };
                                    debug!(
                                        "receive success receive = {},  current = {}",
                                        receive_counter, send_counter
                                    );
                                    if let Ok(message) =
                                        Header::from_bytes(&dns_packet[2..HEADER_SIZE + 2])
                                    {
                                        consumer
                                            .receive(&MessageOrHeader::Header((message, duration)));
                                        receive_counter += 1;
                                    } else {
                                        error!("parse dns message error");
                                        continue 'outer;
                                    }
                                    send_counter += 1;
                                    producer.store.update_query(qtype);
                                    stop_sender_timer = std::time::SystemTime::now();
                                }
                            }
                            Err(e) => {
                                error!("{}", e.to_string());
                                continue 'outer;
                            }
                        }
                    }
                    _ => {}
                }

                if (max_send > 0 && (receive_counter == max_send))
                    || stop_sender_timer.elapsed().unwrap() > std::time::Duration::from_secs(5)
                {
                    debug!(
                        "should break loop send = {} receive = {} cpu={}",
                        send_counter, receive_counter, id
                    );
                    break 'outer;
                }
                if interval != 0 {
                    let now = std::time::SystemTime::now();
                    if now >= next_status_send {
                        producer
                            .store
                            .set_send_duration(now.duration_since(start).unwrap());
                        consumer.store.set_receive_total(receive_counter);
                        consumer.update_report();
                        if let Err(err) =
                            sender.send((producer.store.clone(), consumer.store.clone()))
                        {
                            error!("send interval status fail: {:?}", err)
                        }
                        next_status_send = now.add(std::time::Duration::from_secs(interval));
                    }
                }
            }
            std::mem::drop(sender);
            producer
                .store
                .set_send_duration(stop_sender_timer.duration_since(start).unwrap());
            consumer.store.set_receive_total(receive_counter);
            consumer.receive(&MessageOrHeader::End);
            (producer.store, consumer.store)
        })
    }
}
