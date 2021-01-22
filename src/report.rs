use crate::arguments::Argument;
use chrono::DateTime;
use chrono::Local;
use std::collections::HashMap;
use trust_dns_client::op::ResponseCode;
use trust_dns_client::rr::RecordType;
// use crate::histogram::{HistogramReport};
use crate::histogram::HistogramReport;
use trust_dns_client::op::{Header, Message};

#[derive(Default, Clone)]
pub struct QueryStatusStore {
    total: usize,
    last_update: Option<std::time::SystemTime>,
    query_type: HashMap<u16, usize>,
    answer_type: HashMap<u16, usize>,
    authority_type: HashMap<u16, usize>,
    additional_type: HashMap<u16, usize>,
    reply_code: HashMap<u8, usize>,
    report: Option<HistogramReport>,
}

impl QueryStatusStore {
    pub fn new() -> QueryStatusStore {
        QueryStatusStore {
            total: 0,
            last_update: None,
            query_type: Default::default(),
            answer_type: Default::default(),
            authority_type: Default::default(),
            additional_type: Default::default(),
            reply_code: Default::default(),
            report: None,
        }
    }
    #[allow(dead_code)]
    pub fn update_query(&mut self, query_type: u16) {
        self.total = self.total + 1;
        let count = self.query_type.entry(query_type).or_insert(0);
        *count += 1;
    }

    pub fn update_histogram_report(&mut self, report: Option<HistogramReport>) {
        self.report = report;
    }
    pub fn update_response_from_header(&mut self, header: &Header) {
        // self.total = self.total + 1;
        // only for message type

        // header type only calculate the counter of response code.
        self.total = self.total + 1;
        let r_code = header.response_code();
        let count = self.reply_code.entry(r_code).or_insert(0);
        *count += 1;
        self.last_update = Some(std::time::SystemTime::now());
    }
    pub fn update_response_from_message(&mut self, message: &Message) {
        self.total = self.total + 1;
        let query_type = u16::from(message.queries()[0].query_type());
        let count = self.query_type.entry(query_type).or_insert(0);
        *count += 1;

        for answer in message.answers() {
            let query_type = u16::from(answer.record_type());
            let count = self.answer_type.entry(query_type).or_insert(0);
            *count += 1;
        }

        for answer in message.additionals() {
            let query_type = u16::from(answer.record_type());
            let count = self.additional_type.entry(query_type).or_insert(0);
            *count += 1;
        }

        for answer in message.name_servers() {
            let query_type = u16::from(answer.record_type());
            let count = self.authority_type.entry(query_type).or_insert(0);
            *count += 1;
        }

        let r_code = message.header().response_code();
        let count = self.reply_code.entry(r_code).or_insert(0);
        *count += 1;
        self.last_update = Some(std::time::SystemTime::now());
    }
}

pub struct RunnerReport {
    start: std::time::SystemTime,
    producer_report: Option<QueryStatusStore>,
    consumer_report: Option<QueryStatusStore>,
    histogram: Option<HistogramReport>,
}

impl RunnerReport {
    pub fn new() -> RunnerReport {
        RunnerReport {
            start: std::time::SystemTime::now(),
            producer_report: None,
            consumer_report: None,
            histogram: None,
        }
    }
    pub fn set_producer_report(&mut self, store: QueryStatusStore) {
        self.producer_report = Some(store);
    }
    pub fn set_consumer_report(&mut self, store: QueryStatusStore) {
        self.consumer_report = Some(store);
    }
    pub fn set_histogram_report(&mut self, store: QueryStatusStore) {
        self.histogram = store.report;
    }
    pub fn report(&self, output: impl ReportOutput, arguments: Argument) {
        println!("{}", output.format(&self, arguments))
    }
}

pub trait ReportOutput {
    fn format(&self, report: &RunnerReport, arguments: Argument) -> String;
}

#[allow(dead_code)]
pub enum ReportType {
    Basic,
    Color,
    JSON,
}

struct BasicStats {
    response_code: Vec<(ResponseCode, usize)>,
    start_time: DateTime<Local>,
    end_time: DateTime<Local>,
    query_total: usize,
    response_total: usize,
    qps: f64,
    query_rate: f64,
    min_lantency: f64,
    max_lantency: f64,
    mean_lantency: f64,
    p99: f64,
    p95: f64,
    p90: f64,
    p50: f64,
}
impl BasicStats {
    fn new(report: &RunnerReport) -> BasicStats {
        let response_code =
            format_code_result(&report.consumer_report.as_ref().unwrap().reply_code);

        let start_time: DateTime<Local> = report.start.into();
        let end_time: DateTime<Local> = report
            .consumer_report
            .as_ref()
            .unwrap()
            .last_update
            .expect("thread exit abnormal")
            .into();
        let duration_second = (end_time - start_time).num_milliseconds() as f64 / 1000 as f64;
        let qps = report.consumer_report.as_ref().unwrap().total as f64 / duration_second;
        let query_total = report.producer_report.as_ref().unwrap().total;
        let response_total = report.producer_report.as_ref().unwrap().total;
        let query_rate = report.consumer_report.as_ref().unwrap().total as f64 * 100.0
            / report.producer_report.as_ref().unwrap().total as f64;

        if report.histogram.is_none() {
            BasicStats {
                response_code,
                start_time,
                end_time,
                qps,
                query_total,
                response_total,
                query_rate,
                min_lantency: 0.0,
                max_lantency: 0.0,
                mean_lantency: 0.0,
                p99: 0.0,
                p95: 0.0,
                p90: 0.0,
                p50: 0.0,
            }
        } else {
            let histogram = report.histogram.as_ref().unwrap();
            BasicStats {
                response_code,
                start_time,
                end_time,
                qps,
                query_total,
                response_total,
                query_rate,
                min_lantency: histogram.min,
                max_lantency: histogram.max,
                mean_lantency: histogram.mean,
                p99: histogram.percent99,
                p95: histogram.percent95,
                p90: histogram.percent90,
                p50: histogram.percent50,
            }
        }
    }
}

struct ExtensionStats {
    query_type: Vec<(RecordType, usize)>,
    response_type: Vec<(RecordType, usize, f64)>,
    answer_result: Vec<(RecordType, usize)>,
    additional_result: Vec<(RecordType, usize)>,
    authority_result: Vec<(RecordType, usize)>,
}

impl ExtensionStats {
    fn new(report: &RunnerReport) -> ExtensionStats {
        let mut query_type: Vec<_> = report
            .producer_report
            .as_ref()
            .unwrap()
            .query_type
            .iter()
            .map(|a| (RecordType::from(*a.0), *a.1))
            .collect();
        query_type.sort_by_key(|a| a.0);

        let mut response_type_map: Vec<_> = report
            .consumer_report
            .as_ref()
            .unwrap()
            .query_type
            .iter()
            .collect();
        response_type_map.sort_by_key(|a| a.0);
        let response_type: Vec<_> = response_type_map
            .iter()
            .map(|a| {
                let query_type = RecordType::from(*a.0);
                let rate: f64 = {
                    if let Some(query) =
                        report.producer_report.as_ref().unwrap().query_type.get(a.0)
                    {
                        *a.1 as f64 * 100.0 / *query as f64
                    } else {
                        0.0
                    }
                };
                (query_type, *a.1, rate)
            })
            .collect();

        let answer_result = format_result(&report.consumer_report.as_ref().unwrap().answer_type);

        let additional_result =
            format_result(&report.consumer_report.as_ref().unwrap().additional_type);

        let authority_result =
            format_result(&report.consumer_report.as_ref().unwrap().authority_type);
        ExtensionStats {
            query_type,
            response_type,
            answer_result,
            additional_result,
            authority_result,
        }
    }
}

impl ReportType {
    fn basic(report: &RunnerReport, arguments: Argument) -> String {
        let basic_info = BasicStats::new(report);
        let extension_info = ExtensionStats::new(report);

        let query: Vec<_> = extension_info
            .query_type
            .iter()
            .map(|a| format!("{}={}", a.0.to_string(), a.1))
            .collect();

        let response: Vec<_> = extension_info
            .response_type
            .iter()
            .map(|a| format!("{}={}({:.2}%)", a.0, a.1, a.2))
            .collect();

        let response_code: String = basic_info
            .response_code
            .iter()
            .map(|v| format!("{}={}", v.0, v.1))
            .collect::<Vec<String>>()
            .join(",");

        let answer_result: String = extension_info
            .answer_result
            .iter()
            .map(|v| format!("{}={}", v.0.to_string(), v.1))
            .collect::<Vec<String>>()
            .join(",");

        let additional_result: String = extension_info
            .additional_result
            .iter()
            .map(|v| format!("{}={}", v.0.to_string(), v.1))
            .collect::<Vec<String>>()
            .join(",");

        let authority_result: String = extension_info
            .authority_result
            .iter()
            .map(|v| format!("{}={}", v.0.to_string(), v.1))
            .collect::<Vec<String>>()
            .join(",");

        let mut out_put = format!(
            "------------   Report   --------------
      Total Cost: {} (+time wait)
      Start Time: {}
        End Time: {}
     Total Query: {}
        Question: {}
  Total Response: {}
   Response Code: {}
    Success Rate: {:.2}%
     Average QPS: {:.0}
     Min Latency: {:?}
     Max Latency: {:?}
    Mean Latency: {:?}
     99% Latency: {:?}
     95% Latency: {:?}
     90% Latency: {:?}
     50% Latency: {:?}",
            (basic_info.end_time - basic_info.start_time).to_string(),
            basic_info.start_time.format("%+"),
            basic_info.end_time.format("%+"),
            basic_info.query_total,
            query.join(","),
            basic_info.response_total,
            response_code,
            basic_info.query_rate,
            basic_info.qps,
            std::time::Duration::from_secs_f64(basic_info.min_lantency),
            std::time::Duration::from_secs_f64(basic_info.max_lantency),
            std::time::Duration::from_secs_f64(basic_info.mean_lantency),
            std::time::Duration::from_secs_f64(basic_info.p99),
            std::time::Duration::from_secs_f64(basic_info.p95),
            std::time::Duration::from_secs_f64(basic_info.p90),
            std::time::Duration::from_secs_f64(basic_info.p50),
        );
        if arguments.check_all_message == true {
            let extension_output = format!(
                "
        Question: {}
          Answer: {}
       Authority: {}
      Additional: {}",
                response.join(","),
                answer_result,
                authority_result,
                additional_result
            );
            out_put += extension_output.as_str();
        }
        out_put
    }
    fn color(_report: &RunnerReport) -> String {
        unimplemented!()
    }
    fn json(_report: &RunnerReport) -> String {
        unimplemented!()
    }
}

impl ReportOutput for ReportType {
    fn format(&self, report: &RunnerReport, arguments: Argument) -> String {
        match self {
            ReportType::Basic => ReportType::basic(report, arguments),
            ReportType::Color => ReportType::color(report),
            ReportType::JSON => ReportType::json(report),
        }
    }
}

fn format_result(result_map: &HashMap<u16, usize>) -> Vec<(RecordType, usize)> {
    let mut to_tuple: Vec<_> = result_map.iter().collect();
    to_tuple.sort_by_key(|a| a.0);
    to_tuple
        .iter()
        .map(|a| {
            let query_type = RecordType::from(*a.0);
            (query_type, *a.1)
        })
        .collect::<Vec<(RecordType, usize)>>()
}

fn format_code_result(result_map: &HashMap<u8, usize>) -> Vec<(ResponseCode, usize)> {
    let mut to_tuple: Vec<_> = result_map.iter().collect();
    to_tuple.sort_by_key(|a| a.0);
    to_tuple
        .iter()
        .map(|a| {
            let query_type = trust_dns_client::op::ResponseCode::from(0, *a.0);
            (query_type, *a.1)
        })
        .collect::<Vec<(ResponseCode, usize)>>()
}
