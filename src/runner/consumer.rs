use crate::runner::histogram::Histogram;
use crate::runner::report::StatusStore;
// use crate::utils::{Argument, Protocol};
use crate::workers::MessageOrHeader;

pub struct ResponseConsumer {
    pub store: StatusStore,
    pub histogram: Histogram,
}

impl ResponseConsumer {
    pub fn new() -> ResponseConsumer {
        ResponseConsumer {
            store: StatusStore::new(),
            histogram: Histogram::new(50),
        }
    }
    pub fn receive(&mut self, message: &MessageOrHeader) {
        match message {
            MessageOrHeader::Message((m, elapse)) => {
                self.store.update_response_from_message(&m);
                self.histogram.add(*elapse);
            }
            MessageOrHeader::Header((h, elapse)) => {
                self.store.update_response_from_header(&h);
                self.histogram.add(*elapse);
            }
            MessageOrHeader::End => {
                self.store.update_histogram_report(self.histogram.report());
            }
        }
    }
}
