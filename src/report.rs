use std::collections::HashMap;
use trust_dns_client::op::{Message, OpCode, ResponseCode};
use trust_dns_client::rr::RecordType;

#[derive(Default)]
struct QueryStatusStore {
    total: usize,
    query_type: HashMap<u16, usize>,
    reply_code: HashMap<u16, usize>,
}

impl QueryStatusStore {
    fn update(&mut self, message: &Message) {
        self.total = self.total + 1;
        let qtype = u16::from(message.queries()[0].query_type());
        match self.query_type.get(&qtype) {
            Some(v) => self.query_type.insert(qtype, v + 1),
            _ => self.query_type.insert(qtype, 1),
        };
        let rcode = u16::from(message.response_code());
        match self.reply_code.get(&rcode) {
            Some(v) => self.reply_code.insert(rcode, v + 1),
            _ => self.reply_code.insert(rcode, 1),
        };
    }
}

struct RunnerReport {
    start: std::time::SystemTime,
    end: std::time::SystemTime,
    producer_report: QueryStatusStore,
    consumer_report: QueryStatusStore,
}

impl RunnerReport {
    fn set_start_time(&mut self, begin_at: std::time::SystemTime) {
        self.start = begin_at;
    }
    fn set_end_time(&mut self, end_at: std::time::SystemTime) {
        self.end = end_at;
    }
    fn set_producer_report(&mut self, store: QueryStatusStore) {
        self.producer_report = store;
    }
    fn set_consumer_report(&mut self, store: QueryStatusStore) {
        self.consumer_report = store;
    }
    fn report(&self, output: impl ReportOutput) -> String {
        output.format(&self)
    }
}

trait ReportOutput {
    fn format(&self, report: &RunnerReport) -> String;
}

enum ReportType {
    Basic,
    JSON,
    Color,
}
impl ReportType {
    fn basic(report: &RunnerReport) -> String {
        format!("basic output")
    }
    fn json(report: &RunnerReport) -> String {
        format!("json output")
    }
    fn color(report: &RunnerReport) -> String {
        format!("color output")
    }
}

impl ReportOutput for ReportType {
    fn format(&self, report: &RunnerReport) -> String {
        match self {
            ReportType::Basic => ReportType::basic(report),
            ReportType::JSON => ReportType::json(report),
            ReportType::Color => ReportType::color(report),
            _ => unimplemented!(),
        }
    }
}
