use crate::arguments::Argument;
use std::sync::Arc;

pub struct Runner<'a> {
    arguments: &'a Argument,
    workers: Vec<QueryWorker>,
}

impl<'a> Runner<'a> {
    pub(crate) fn new(arguments: &'a mut Argument) -> Runner {
        let mut workers = Vec::new();
        let qps = {
            let qps = arguments.qps / arguments.client;
            if qps == 0 {
                1
            } else {
                qps
            }
        };
        arguments.qps = qps;
        let origin_arguments = Arc::new(arguments.clone());
        for i in 0..origin_arguments.client {
            let argument = origin_arguments.clone();
            workers.push(QueryWorker::new(i, argument));
        }
        Runner { arguments, workers }
    }
}

impl<'a> Drop for Runner<'a> {
    fn drop(&mut self) {
        for worker in &mut self.workers {
            if let Some(handler) = worker.thread.take() {
                handler.join().expect("fail to join thread");
            }
        }
    }
}

struct QueryWorker {
    id: usize,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl QueryWorker {
    fn new(id: usize, argument: Arc<Argument>) -> QueryWorker {
        let thread = std::thread::spawn(move || {
            // gen_traffic(argument);
            // TODO
            return;
        });
        QueryWorker {
            id,
            thread: Some(thread),
        }
    }
}
