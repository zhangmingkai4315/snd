use crate::arguments::Argument;
use crate::cache::Cache;
use futures::Future;
use governor::{Quota, RateLimiter};
use nonzero_ext::*;
use std::num::NonZeroU32;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use tokio::time::Duration;
use trust_dns_client::op::Message;

use futures::future::join_all;
use std::error::Error;
use std::iter::Product;
use tokio::macros::support::thread_rng_n;

pub struct Runner {
    arguments: Argument,
    workers: Vec<QueryWorker>,
    producer: QueryProducer,
}

impl Runner {
    pub(crate) fn new(arguments: Argument) -> Runner {
        let mut workers = Vec::new();
        let (sender, receiver) = channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let origin_arguments = Arc::new(arguments.clone());
        for i in 0..origin_arguments.client {
            workers.push(QueryWorker::new(i, Arc::clone(&receiver)));
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
    fn new(argument: Argument, sender: std::sync::mpsc::Sender<Vec<u8>>) -> QueryProducer {
        let sender = sender.clone();
        let mut cache = Cache::new(&argument.clone());
        let thread = std::thread::spawn(move || {
            let rate_limiter = RateLimiter::direct(Quota::per_second(
                NonZeroU32::new(argument.qps as u32).expect("qps setting error"),
            ));
            loop {
                if rate_limiter.check().is_ok() {
                    sender.send(cache.build_message());
                }
            }
        });
        QueryProducer {
            // argument: argument.clone(),
            thread,
        }
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
        let thread = std::thread::spawn(move || loop {
            let data = rx.lock().unwrap().recv().unwrap();
            println!("{:?}", data);
        });
        QueryWorker {
            id,
            receiver,
            thread: Some(thread),
        }
    }
}
