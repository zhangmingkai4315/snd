use crate::runner::report::{RunnerReport, StatusStore};
use crate::utils::utils::cpu_mode_to_cpu_cores;
use crate::utils::{Argument, Protocol};
use crate::workers::tcp::TCPWorker;
use crate::workers::{
    // doh::DOHWorker, tcp::TCPWorker,  udp_async::UDPAsyncWorker,dot::DoTWorker,
    udp::UDPWorker,
    Worker,
};
use core_affinity;
use core_affinity::CoreId;
use std::collections::HashMap;
use std::hash::Hash;

pub struct Runner {
    arguments: Argument,
    workers: Vec<(Box<dyn Worker>, CoreId)>,
    report: RunnerReport,
}

impl Runner {
    pub fn new(arguments: Argument) -> Result<Runner, String> {
        let protocol = arguments.protocol.clone();
        let worker_factory: fn(Argument) -> Box<dyn Worker> = match protocol {
            Protocol::TCP => TCPWorker::new,
            _ => UDPWorker::new,
        };
        let mut workers: std::vec::Vec<(
            std::boxed::Box<(dyn Worker + 'static)>,
            core_affinity::CoreId,
        )> = vec![];
        let bind_cpu = arguments.bind_cpu.clone();
        match cpu_mode_to_cpu_cores(bind_cpu) {
            Err(e) => {
                return Err(format!("{}", e.to_string()));
            }
            Ok(v) => {
                debug!("bind worker threads to cpu cores: {:?}", v);
                let core_number = v.len();
                for (index, core_id) in v.iter().enumerate() {
                    let mut args = arguments.clone();
                    if args.qps != 0 {
                        args.qps = args.qps / core_number + {
                            if index == 0 {
                                args.qps % core_number
                            } else {
                                0
                            }
                        };
                    }
                    if args.max != 0 {
                        args.max = args.max / core_number + {
                            if index == 0 {
                                args.max % core_number
                            } else {
                                0
                            }
                        };
                    }
                    if args.client != 0 {
                        args.client = args.client / core_number + {
                            if index == 0 {
                                args.client % core_number
                            } else {
                                0
                            }
                        }
                    }
                    workers.push((worker_factory(args), *core_id));
                }
            }
        }
        Ok(Runner {
            arguments: arguments.clone(),
            report: RunnerReport::new(),
            workers,
        })
    }
    pub fn run(&mut self) {
        debug!("start runner and generate many threads");
        let worker_number = self.workers.len();
        let mut query_store_total = StatusStore::new();
        let mut response_store_total = StatusStore::new();

        let (snd, rcv) = crossbeam_channel::unbounded();
        let (interval_snd, interval_rcv) = crossbeam_channel::bounded(worker_number);
        if let Err(e) = crossbeam::scope(|s| {
            let mut handles = vec![];
            for (worker, id) in &mut self.workers {
                let interval_snd = interval_snd.clone();
                handles.push(s.spawn(move |_| {
                    core_affinity::set_for_current(id.clone());
                    worker.run(id.id, interval_snd)
                }));
            }
            drop(interval_snd);
            s.spawn(|_| {
                debug!("start collection interval data");
                'outer: loop {
                    let mut interval_query_store_total = StatusStore::new();
                    let mut interval_response_store_total = StatusStore::new();
                    let mut report = RunnerReport::new();

                    for _ in 0..worker_number {
                        match interval_rcv.recv() {
                            Ok(status) => {
                                interval_query_store_total =
                                    interval_query_store_total + status.0.clone();
                                interval_response_store_total =
                                    interval_response_store_total + status.1.clone();
                            }
                            _ => break 'outer,
                        }
                    }
                    report.set_producer_report(interval_query_store_total);
                    report.set_consumer_report(interval_response_store_total.clone());
                    report.set_histogram_report(interval_response_store_total);
                    report.report("stdout".to_string());
                }
            });

            for i in handles {
                let status = i.join().unwrap();
                if let Err(e) = snd.send(status) {
                    error!("send status to channel fail: {:?}", e.to_string());
                };
            }
        }) {
            error!("thread create fail: {:?}", e);
        };
        for _ in 0..worker_number {
            let status = rcv.recv().unwrap();
            query_store_total = query_store_total + status.0;
            response_store_total = response_store_total + status.1;
        }

        // let (query_store_total, response_store_total) = self.worker.run();
        self.report.set_producer_report(query_store_total);
        self.report
            .set_consumer_report(response_store_total.clone());
        self.report.set_histogram_report(response_store_total);
        self.report.report(self.arguments.output.clone());
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
