use crate::runner::report::StatusStore;
use trust_dns_client::op::{Header, Message};

pub trait Worker: Send + Sync {
    fn run(
        &mut self,
        id: usize,
        sender: crossbeam_channel::Sender<(StatusStore, StatusStore)>,
    ) -> (StatusStore, StatusStore);
}

const HEADER_SIZE: usize = 12;

pub enum MessageOrHeader {
    Message((Message, f64)),
    Header((Header, f64)),
    End,
}

pub mod doh;
// pub mod dot;
pub mod tcp;
pub mod udp;
