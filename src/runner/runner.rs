use crate::runner::report::RunnerReport;
use crate::utils::Argument;
use crate::workers::{
    // doh::DOHWorker, tcp::TCPWorker,  udp_async::UDPAsyncWorker,dot::DoTWorker,
    udp::UDPWorker,
    Worker,
};
use std::collections::HashMap;
use std::hash::Hash;

pub struct Runner {
    arguments: Argument,
    worker: Box<dyn Worker>,
    report: RunnerReport,
}

impl Runner {
    pub fn new(arguments: Argument) -> Runner {
        // let (sender, receiver) = channel();
        // let (sender, receiver) = bounded(arguments.client);
        // let receiver = Arc::new(Mutex::new(receiver));
        // let origin_arguments = Arc::new(arguments.clone());
        // let (status_sender, status_receiver) = bounded(arguments.client);
        // let query_status_sender = status_sender.clone();
        match arguments.protocol {
            _ => {
                Runner {
                    arguments: arguments.clone(),
                    report: RunnerReport::new(),
                    worker: Box::new(UDPWorker::new(arguments.clone())),
                }
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
    pub fn run(&mut self) {
        let (query_store_total, response_store_total) = self.worker.run();
        self.report.set_producer_report(query_store_total);
        self.report
            .set_consumer_report(response_store_total.clone());
        self.report.set_histogram_report(response_store_total);
        self.report.report(self.arguments.clone());
    }
}

pub fn merge_map<K>(first: &HashMap<K, u64>, second: &HashMap<K, u64>) -> HashMap<K, u64>
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
