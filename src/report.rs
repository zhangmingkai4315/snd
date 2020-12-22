use chrono::DateTime;
use chrono::Local;
use std::collections::HashMap;
use trust_dns_client::op::Message;
use trust_dns_client::rr::RecordType;

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
    fn basic(report: &RunnerReport) -> String {
        let mut query_type_map: Vec<_> = report
            .producer_report
            .as_ref()
            .unwrap()
            .query_type
            .iter()
            .collect();
        query_type_map.sort_by_key(|a| a.0);
        let query: Vec<_> = query_type_map
            .iter()
            .map(|a| format!("{} = {}", RecordType::from(*a.0).to_string(), a.1))
            .collect();

        let mut response_type_map: Vec<_> = report
            .consumer_report
            .as_ref()
            .unwrap()
            .query_type
            .iter()
            .collect();
        response_type_map.sort_by_key(|a| a.0);
        let response: Vec<_> = response_type_map
            .iter()
            .map(|a| {
                let qtype = RecordType::from(*a.0).to_string();
                let rate: f64 = {
                    if let Some(query) =
                        report.producer_report.as_ref().unwrap().query_type.get(a.0)
                    {
                        *a.1 as f64 * 100.0 / *query as f64
                    } else {
                        0.0
                    }
                };
                format!("{} = {}({:.2}%)", qtype, a.1, rate)
            })
            .collect();
        let start_time: DateTime<Local> = report.start.into();
        let end_time: DateTime<Local> = report.end.unwrap().into();
        format!(
            "\
------------ Report  -----------
start time: {}\n\
end time: {}\n\
total cost: {:?}\n\
total query: {} \n\t{}\n
total response: {} \n\t{}\n
response rate {:.2}%\n",
            start_time.format("%+"),
            end_time.format("%+"),
            report.end.unwrap().duration_since(report.start).unwrap(),
            report.producer_report.as_ref().unwrap().total,
            query.join(","),
            report.consumer_report.as_ref().unwrap().total,
            response.join(","),
            report.consumer_report.as_ref().unwrap().total as f64 * 100.0
                / report.producer_report.as_ref().unwrap().total as f64,
        )
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
