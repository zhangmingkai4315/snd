use crate::runner::report::StatusStore;
use trust_dns_client::op::{Header, Message};

pub trait Worker {
    fn run(&mut self) -> (StatusStore, StatusStore);
}

const HEADER_SIZE: usize = 12;

pub enum MessageOrHeader {
    Message((Message, f64)),
    Header((Header, f64)),
    End,
}

// pub mod doh;
// pub mod dot;
// pub mod tcp;
pub mod udp;
// pub mod udp_async;
