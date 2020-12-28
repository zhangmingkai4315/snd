pub trait Worker {
    fn block(&mut self) {}
}

pub mod tcp;
pub mod udp;
pub mod doh;

