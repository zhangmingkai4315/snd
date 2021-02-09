use crate::arguments::{Argument, Protocol};
use crate::cache::Cache;
use crate::histogram::Histogram;
use crate::report::{QueryStatusStore, RunnerReport};
use crate::workers::dot::DoTWorker;
use crate::workers::{doh::DOHWorker, tcp::TCPWorker, udp::UDPWorker, udp_async::UDPAsyncWorker, MessageOrHeader, Worker};
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};

pub struct Runner {
    arguments: Argument,
    workers: Vec<Box<dyn Worker>>,
    producer: QueryProducer,
    consumer: QueryConsumer,
    report: RunnerReport,
    result_sender: Sender<MessageOrHeader>,
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
                    workers.push(Box::new(UDPAsyncWorker::new(
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
                Protocol::DOT => {
                    workers.push(Box::new(DoTWorker::new(
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
            arguments,
            workers,
            producer,
            consumer,
            result_sender: result_sender.clone(),
            report: RunnerReport::new(),
        }
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        for worker in &mut self.workers {
            worker.block();
        }
        // send a end signal
        self.result_sender
            .send(MessageOrHeader::End)
            .expect("send end signal error");
        self.report
            .set_producer_report((*self.producer.store.lock().unwrap()).clone());
        self.report
            .set_consumer_report((*self.consumer.store.lock().unwrap()).clone());
        // block until receive close channel
        self.consumer
            .close_receiver
            .recv()
            .expect("receive close channel fail");
        self.report
            .set_histogram_report((*self.consumer.store.lock().unwrap()).clone());
        self.report.report(self.arguments.clone());
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
        let store = Arc::new(Mutex::new(QueryStatusStore::new()));
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

struct QueryConsumer {
    close_receiver: Receiver<bool>,
    store: Arc<Mutex<QueryStatusStore>>,
}

impl QueryConsumer {
    fn new(receiver: Receiver<MessageOrHeader>) -> QueryConsumer {
        let thread_receiver = receiver.clone();
        let store = Arc::new(Mutex::new(QueryStatusStore::default()));
        let thread_store = store.clone();
        let (close_sender, close_receiver) = unbounded();
        std::thread::spawn(move || {
            let ref mut histogram = Histogram::new(50);
            for _message in thread_receiver {
                match thread_store.lock() {
                    Ok(mut v) => match &_message {
                        MessageOrHeader::Message((m, elapse)) => {
                            v.update_response_from_message(&m);
                            histogram.add(*elapse);
                        }
                        MessageOrHeader::Header((h, elapse)) => {
                            v.update_response_from_header(&h);
                            histogram.add(*elapse);
                        }
                        MessageOrHeader::End => {
                            v.update_histogram_report(histogram.report());
                            close_sender.send(true).expect("send to close channel fail");
                        }
                    },
                    _ => error!("lock store thread fail"),
                }
            }
        });

        QueryConsumer {
            store,
            close_receiver: close_receiver.clone(),
        }
    }
}
