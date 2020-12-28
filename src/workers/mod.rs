use trust_dns_client::op::{Header, Message};

pub trait Worker {
    fn block(&mut self) {}
}
const HEADER_SIZE: usize = 12;

pub enum MessageOrHeader {
    Message(Message),
    Header(Header),
}

pub mod doh;
pub mod tcp;
pub mod udp;
