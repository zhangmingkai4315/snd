use crate::arguments::Argument;
use crate::cache::Cache;
use crossbeam_channel::{bounded, Receiver, Sender};
use futures::future::join_all;
use futures::Future;
use governor::{Quota, RateLimiter};
use nonzero_ext::*;
use std::error::Error;
use std::iter::Product;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use tokio::macros::support::thread_rng_n;
use tokio::time::Duration;
use trust_dns_client::op::Message;

pub struct Runner {
    arguments: Argument,
    workers: Vec<QueryWorker>,
    producer: QueryProducer,
}

impl Runner {
    pub(crate) fn new(arguments: Argument) -> Runner {
        let mut workers = Vec::new();
        // let (sender, receiver) = channel();
        let (sender, receiver) = bounded(arguments.client);
        let receiver = Arc::new(Mutex::new(receiver));
        let origin_arguments = Arc::new(arguments.clone());
        for i in 0..origin_arguments.client {
            workers.push(QueryWorker::new(i, receiver.clone()));
        }
        let producer = QueryProducer::new(arguments.clone(), sender.clone());
        Runner {
            arguments,
            workers,
            producer,
        }
    }

    pub fn run(&self) {}
}

impl Drop for Runner {
    fn drop(&mut self) {
        for worker in &mut self.workers {
            if let Some(handler) = worker.thread.take() {
                handler.join().expect("fail to join thread");
            }
        }
    }
}

struct QueryProducer {
    // argument: Argument,
    thread: std::thread::JoinHandle<()>,
    // sender: std::sync::mpsc::Sender<Vec<u8>>
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
                Some(RateLimiter::direct(Quota::per_second(
                    NonZeroU32::new(argument.qps as u32).expect("qps setting error"),
                )))
            }
        };
        let mut current_counter: usize = 0;
        let mut cache = Cache::new(&argument.clone());
        let thread = std::thread::spawn(move || {
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
                        println!("{}", current_counter);
                        current_counter = current_counter + 1;
                        sender.send(cache.build_message());
                    } else {
                        break;
                    }
                }
            }
            info!("producer thread quit ");
            drop(sender);
        });
        QueryProducer { thread }
    }
}

struct QueryWorker {
    id: usize,
    receiver: Arc<Mutex<Receiver<Vec<u8>>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl QueryWorker {
    fn new(id: usize, receiver: Arc<Mutex<Receiver<Vec<u8>>>>) -> QueryWorker {
        let rx = receiver.clone();
        let thread = std::thread::spawn(move || {
            loop {
                match rx.lock().unwrap().recv() {
                    Ok(data) => {
                        println!("{:?}", data);
                    }
                    Err(e) => {
                        break;
                    }
                };
            }
            info!("worker thread {} exit success", id)
        });
        QueryWorker {
            id,
            receiver,
            thread: Some(thread),
        }
    }
}
