use std::collections::HashMap;
use trust_dns_client::op::Message;

#[derive(Default, Clone)]
pub struct QueryStatusStore {
    total: usize,
    query_type: HashMap<u16, usize>,
    reply_code: HashMap<u16, usize>,
}

impl QueryStatusStore {
    pub fn update_query(&mut self, qtype: u16) {
        self.total = self.total + 1;
        match self.query_type.get(&qtype) {
            Some(v) => self.query_type.insert(qtype, v + 1),
            _ => self.query_type.insert(qtype, 1),
        };
    }

    pub fn update(&mut self, message: &Message) {
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

pub struct RunnerReport {
    start: std::time::SystemTime,
    end: Option<std::time::SystemTime>,
    producer_report: Option<QueryStatusStore>,
    consumer_report: Option<QueryStatusStore>,
}

impl RunnerReport {
    pub fn new() -> RunnerReport {
        RunnerReport {
            start: std::time::SystemTime::now(),
            end: None,
            producer_report: None,
            consumer_report: None,
        }
    }
    pub fn set_start_time(&mut self, begin_at: std::time::SystemTime) {
        self.start = begin_at;
    }
    pub fn set_end_time(&mut self, end_at: std::time::SystemTime) {
        self.end = Some(end_at);
    }
    pub fn set_producer_report(&mut self, store: QueryStatusStore) {
        self.producer_report = Some(store);
    }
    pub fn set_consumer_report(&mut self, store: QueryStatusStore) {
        self.consumer_report = Some(store);
    }
    pub fn report(&self, output: impl ReportOutput) {
        println!("{}", output.format(&self))
    }
}

pub trait ReportOutput {
    fn format(&self, report: &RunnerReport) -> String;
}

pub enum ReportType {
    Basic,
    JSON,
    Color,
}
impl ReportType {
    fn basic(_report: &RunnerReport) -> String {
        format!("basic output")
    }
    fn json(_report: &RunnerReport) -> String {
        format!("json output")
    }
    fn color(_report: &RunnerReport) -> String {
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
