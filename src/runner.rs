use crate::arguments::Argument;
use std::sync::Arc;

pub struct Runner {
    arguments: Arc<Argument>,
    workers: Vec<Worker>,
}

impl Runner {
    pub(crate) fn new(arguments: Argument) -> Runner {
        let mut workers = Vec::new();
        let arguments = Arc::new(arguments);
        for i in 0..arguments.client {
            workers.push(Worker::new(i, arguments.clone()));
        }
        Runner { arguments, workers }
    }
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

struct Worker {
    id: usize,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Worker {
    fn new(id: usize, argument: Arc<Argument>) -> Worker {
        let thread = std::thread::spawn(move || {
            // gen_traffic(argument);
            println!("{:?}", argument);
            return;
        });
        Worker {
            id,
            thread: Some(thread),
        }
    }
}
