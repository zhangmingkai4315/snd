use crossbeam_channel::{bounded, Receiver};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use crate::runner::report::{RunnerReport, StatusStore};
use crate::utils::Argument;
use crate::workers::{
    // doh::DOHWorker, tcp::TCPWorker,  udp_async::UDPAsyncWorker,dot::DoTWorker,
    udp::UDPWorker,
    Worker,
};

pub struct Runner {
    arguments: Argument,
    workers: Vec<Box<dyn Worker>>,
    report: RunnerReport,
    query_status_receiver: Receiver<(StatusStore, StatusStore)>,
}

impl Runner {
    pub fn new(arguments: Argument) -> Runner {
        let mut workers: Vec<Box<dyn Worker>> = Vec::new();
        // let (sender, receiver) = channel();
        // let (sender, receiver) = bounded(arguments.client);
        // let receiver = Arc::new(Mutex::new(receiver));
        let origin_arguments = Arc::new(arguments.clone());
        let (status_sender, status_receiver) = bounded(arguments.client);
        if origin_arguments.enable_async {
            // workers.push(Box::new(UDPAsyncWorker::new(
            //     arguments.clone(),
            //     receiver.clone(),
            //     result_sender.clone(),
            // )));
        } else {
            for _i in 0..origin_arguments.client {
                let query_status_sender = status_sender.clone();
                match arguments.protocol {
                    _ => {
                        workers.push(Box::new(UDPWorker::new(
                            arguments.clone(),
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
        Runner {
            arguments,
            workers,
            query_status_receiver: status_receiver.clone(),
            report: RunnerReport::new(),
        }
    }

    pub fn run(&mut self) {
        let mut workers_threads = vec![];
        for worker in &mut self.workers {
            workers_threads.push(worker.run());
        }
        for work in workers_threads {
            if let Err(e) = work.join() {
                panic!("join thread fail: {:?}", e)
            }
        }
        // merge query store

        let mut query_store_total = StatusStore::new();
        let mut response_store_total = StatusStore::new();
        for _ in 1..=self.workers.len() {
            if let Ok(store) = self.query_status_receiver.recv() {
                query_store_total = query_store_total + store.0;
                response_store_total = response_store_total + store.1;
            }
        }
        // how to
        self.report.set_producer_report(query_store_total);
        self.report
            .set_consumer_report(response_store_total.clone());
        self.report.set_histogram_report(response_store_total);
        self.report.report(self.arguments.clone());
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
