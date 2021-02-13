use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use governor::{Quota, RateLimiter};
use std::collections::HashMap;
use std::hash::Hash;
use std::num::NonZeroU32;
use std::ops::{Add, Deref};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::utils::{Argument, Protocol};
use crate::runner::cache::Cache;
use crate::runner::histogram::Histogram;
use crate::runner::report::{QueryStatusStore, RunnerReport};
use crate::workers::{
    // doh::DOHWorker, tcp::TCPWorker,  udp_async::UDPAsyncWorker,dot::DoTWorker,
    udp::UDPWorker,
    MessageOrHeader,
    Worker,
};



pub struct Runner {
    arguments: Argument,
    workers: Vec<Box<dyn Worker>>,
    consumer: QueryConsumer,
    report: RunnerReport,
    result_sender: Sender<MessageOrHeader>,
    query_status_receiver: Receiver<QueryStatusStore>,
}

impl Runner {
    pub fn new(arguments: Argument) -> Runner {
        let mut workers: Vec<Box<dyn Worker>> = Vec::new();
        // let (sender, receiver) = channel();
        // let (sender, receiver) = bounded(arguments.client);
        // let receiver = Arc::new(Mutex::new(receiver));
        let origin_arguments = Arc::new(arguments.clone());
        let (status_sender, status_receiver) = bounded(arguments.client);
        let (result_sender, result_receiver) = bounded(arguments.client);
        if origin_arguments.enable_async {
            // workers.push(Box::new(UDPAsyncWorker::new(
            //     arguments.clone(),
            //     receiver.clone(),
            //     result_sender.clone(),
            // )));
        } else {
            for _i in 0..origin_arguments.client {
                let result_sender = result_sender.clone();
                let query_status_sender = status_sender.clone();
                match arguments.protocol {
                    _ => {
                        workers.push(Box::new(UDPWorker::new(
                            arguments.clone(),
                            result_sender,
                            query_status_sender,
                        )));
                    }
                    // Protocol::TCP => {
                    //     workers.push(Box::new(TCPWorker::new(
                    //         arguments.clone(),
                    //         receiver.clone(),
                    //         result_sender,
                    //     )));
                    // }
                    // Protocol::DOT => {
                    //     workers.push(Box::new(DoTWorker::new(
                    //         arguments.clone(),
                    //         receiver.clone(),
                    //         result_sender,
                    //     )));
                    // }
                    // Protocol::DOH => workers.push(Box::new(DOHWorker::new(
                    //     arguments.clone(),
                    //     receiver.clone(),
                    //     result_sender,
                    // ))),
                }
            }
        }

        let consumer = QueryConsumer::new(result_receiver);
        Runner {
            arguments,
            workers,
            consumer,
            result_sender: result_sender.clone(),
            query_status_receiver: status_receiver,
            report: RunnerReport::new(),
        }
    }

    pub fn run(&mut self) {
        let mut workers_threads = vec![];
        for worker in &mut self.workers {
            workers_threads.push(worker.run());
        }
        for work in workers_threads {
            work.join();
        }
        // merge query store

        let mut query_store_total = QueryStatusStore::new();
        for i in 1..=self.workers.len() {
            if let Ok(mut query_store) = self.query_status_receiver.recv() {
                query_store_total = query_store_total + query_store;
            }
        }
        // send a end signal
        self.result_sender
            .send(MessageOrHeader::End)
            .expect("send end signal error");
        // how to

        self.report.set_producer_report(query_store_total);
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

pub struct QueryConsumer {
    close_receiver: Receiver<bool>,
    store: Arc<Mutex<QueryStatusStore>>,
}

impl QueryConsumer {
    pub fn new(receiver: Receiver<MessageOrHeader>) -> QueryConsumer {
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

pub fn merge_map<K>(first: &HashMap<K, usize>, second: &HashMap<K, usize>) -> HashMap<K, usize>
where
    K: Clone + Eq + Hash,
{
    let mut result = HashMap::new();
    for (k, v) in first {
        result.insert(k.clone(), v.clone());
    }
    for (k, v) in second {
        let count = result.entry(k.clone()).or_insert(0);
        *count += v;
    }
    return result;
}
