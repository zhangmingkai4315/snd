use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::ops::Add;
use trust_dns_client::op::ResponseCode;
use trust_dns_client::op::{Header, Message};
use trust_dns_client::rr::RecordType;
// use crate::histogram::{HistogramReport};
use crate::runner::histogram::HistogramReport;
use crate::runner::runner::merge_map;

#[derive(Default, Clone, Debug)]
pub struct StatusStore {
    query_total: u64,
    receive_total: u64,
    send_duration: Option<std::time::Duration>,
    last_update: Option<std::time::SystemTime>,
    query_type: HashMap<u16, u64>,
    answer_type: HashMap<u16, u64>,
    authority_type: HashMap<u16, u64>,
    additional_type: HashMap<u16, u64>,
    reply_code: HashMap<u8, u64>,
    report: Option<HistogramReport>,
}

impl Add<StatusStore> for StatusStore {
    type Output = StatusStore;

    fn add(self, rhs: StatusStore) -> Self::Output {
        Self {
            query_total: self.query_total + rhs.query_total,
            receive_total: self.receive_total + rhs.receive_total,
            send_duration: {
                match (self.send_duration, rhs.send_duration) {
                    (Some(v1), Some(v2)) => {
                        if v1 > v2 {
                            Some(v1)
                        } else {
                            Some(v2)
                        }
                    }
                    (Some(v1), None) => Some(v1),
                    (None, Some(v2)) => Some(v2),
                    _ => None,
                }
            },
            last_update: {
                match (self.last_update, rhs.last_update) {
                    (Some(v1), Some(v2)) => {
                        if v1 > v2 {
                            Some(v1)
                        } else {
                            Some(v2)
                        }
                    }
                    (Some(v1), None) => Some(v1),
                    (None, Some(v2)) => Some(v2),
                    _ => None,
                }
            },
            query_type: merge_map(&self.query_type, &rhs.query_type),
            answer_type: merge_map(&self.answer_type, &rhs.answer_type),
            authority_type: merge_map(&self.authority_type, &rhs.authority_type),
            additional_type: merge_map(&self.additional_type, &rhs.additional_type),
            reply_code: merge_map(&self.reply_code, &rhs.reply_code),
            report: {
                match (&self.report, &rhs.report) {
                    (Some(v1), Some(v2)) => Some(v1.clone() + v2.clone()),
                    (None, Some(v2)) => Some(v2.clone()),
                    (Some(v1), None) => Some(v1.clone()),
                    _ => None,
                }
            },
        }
    }
}

impl StatusStore {
    pub fn new() -> StatusStore {
        StatusStore {
            query_total: 0,
            receive_total: 0,
            send_duration: None,
            last_update: None,
            query_type: Default::default(),
            answer_type: Default::default(),
            authority_type: Default::default(),
            additional_type: Default::default(),
            reply_code: Default::default(),
            report: None,
        }
    }
    pub fn new_from_query_status(query_status: HashMap<u16, u64>) -> StatusStore {
        let mut query_total: u64 = 0;
        for (_, v) in query_status.clone() {
            query_total += v;
        }
        StatusStore {
            query_total: query_total,
            receive_total: 0,
            send_duration: None,
            last_update: Some(std::time::SystemTime::now()),
            query_type: query_status.clone(),
            answer_type: Default::default(),
            authority_type: Default::default(),
            additional_type: Default::default(),
            reply_code: Default::default(),
            report: None,
        }
    }
    pub fn set_query_total(&mut self, total: u64) {
        self.query_total = total;
    }
    pub fn set_receive_total(&mut self, total: u64) {
        self.receive_total = total;
    }
    pub fn set_send_duration(&mut self, duration: std::time::Duration) {
        self.send_duration = Some(duration);
    }
    #[allow(dead_code)]
    // update from producer
    pub fn update_query(&mut self, query_type: u16) {
        self.query_total = self.query_total + 1;
        let count = self.query_type.entry(query_type).or_insert(0);
        *count += 1;
    }
    pub fn get_query(&mut self) -> HashMap<u16, u64> {
        self.query_type.clone()
    }

    pub fn update_histogram_report(&mut self, report: Option<HistogramReport>) {
        self.report = report;
    }
    pub fn update_response_from_header(&mut self, header: &Header) {
        let r_code = header.response_code();
        let count = self.reply_code.entry(r_code).or_insert(0);
        *count += 1;
        self.last_update = Some(std::time::SystemTime::now());
    }
    pub fn update_response_from_message(&mut self, message: &Message) {
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
    producer_report: Option<StatusStore>,
    consumer_report: Option<StatusStore>,
    histogram: Option<HistogramReport>,
}

impl RunnerReport {
    pub fn new() -> RunnerReport {
        RunnerReport {
            producer_report: None,
            consumer_report: None,
            histogram: None,
        }
    }
    pub fn set_producer_report(&mut self, store: StatusStore) {
        self.producer_report = Some(store);
    }
    pub fn set_consumer_report(&mut self, store: StatusStore) {
        self.consumer_report = Some(store);
    }
    pub fn set_histogram_report(&mut self, store: StatusStore) {
        self.histogram = store.report;
    }

    pub fn report(&self, target: String) {
        let mut output = ReportType::Basic;
        let report_file_name = target.to_ascii_lowercase();
        if report_file_name.ends_with(".json") {
            output = ReportType::JSON
        } else if report_file_name.ends_with(".yaml") {
            output = ReportType::YAML
        }
        output.format(self, target)
    }
}

pub trait ReportOutput {
    fn format(&self, report: &RunnerReport, target: String);
}

#[allow(dead_code)]
pub enum ReportType {
    Basic,
    // TOML,
    YAML,
    JSON,
}

struct BasicStats {
    response_code: Vec<(ResponseCode, u64)>,
    duration: std::time::Duration,
    query_total: u64,
    response_total: u64,
    qps: u64,
    query_rate: f64,
    min_lantency: f64,
    max_lantency: f64,
    mean_lantency: f64,
    p99: f64,
    p95: f64,
    p90: f64,
    p50: f64,
}

#[derive(Serialize, Deserialize, Debug)]
struct BasicStatsSerializable {
    response_code: Vec<ItemKeyValue>,
    duration: std::time::Duration,
    query_total: u64,
    response_total: u64,
    qps: u64,
    query_rate: f64,
    min_lantency: f64,
    max_lantency: f64,
    mean_lantency: f64,
    p99: f64,
    p95: f64,
    p90: f64,
    p50: f64,
}
#[derive(Serialize, Deserialize, Debug)]
struct ItemKeyValue {
    key: String,
    value: u64,
}
#[derive(Serialize, Deserialize, Debug)]
struct ItemKeyValueRate {
    key: String,
    value: u64,
    rate: f64,
}

impl BasicStats {
    fn to_serializable(&self) -> BasicStatsSerializable {
        BasicStatsSerializable {
            response_code: self
                .response_code
                .iter()
                .map(|a| ItemKeyValue {
                    key: a.0.to_string(),
                    value: a.1,
                })
                .collect(),
            duration: self.duration,
            query_total: self.query_total,
            response_total: self.response_total,
            qps: self.qps,
            query_rate: self.query_rate,
            min_lantency: self.min_lantency,
            max_lantency: self.max_lantency,
            mean_lantency: self.mean_lantency,
            p99: self.p99,
            p95: self.p95,
            p90: self.p90,
            p50: self.p50,
        }
    }
    fn new(report: &RunnerReport) -> BasicStats {
        let response_code =
            format_code_result(&report.consumer_report.as_ref().unwrap().reply_code);

        let duration = report
            .producer_report
            .as_ref()
            .unwrap()
            .send_duration
            .unwrap();
        let qps = (report.producer_report.as_ref().unwrap().query_total as f64
            / duration.as_secs_f64()) as u64;
        let query_total = report.producer_report.as_ref().unwrap().query_total;
        let response_total = report.consumer_report.as_ref().unwrap().receive_total;
        let query_rate = report.consumer_report.as_ref().unwrap().receive_total as f64 * 100.0
            / report.producer_report.as_ref().unwrap().query_total as f64;

        if report.histogram.is_none() {
            BasicStats {
                response_code,
                duration,
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
                duration,
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
    query_type: Vec<(RecordType, u64)>,
    response_type: Vec<(RecordType, u64, f64)>,
    answer_result: Vec<(RecordType, u64)>,
    additional_result: Vec<(RecordType, u64)>,
    authority_result: Vec<(RecordType, u64)>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ExtensionStatsSerializable {
    query_type: Vec<ItemKeyValue>,
    response_type: Vec<ItemKeyValueRate>,
    answer_result: Vec<ItemKeyValue>,
    additional_result: Vec<ItemKeyValue>,
    authority_result: Vec<ItemKeyValue>,
}

impl ExtensionStats {
    fn to_serializable(&self) -> ExtensionStatsSerializable {
        ExtensionStatsSerializable {
            query_type: self
                .query_type
                .iter()
                .map(|a| ItemKeyValue {
                    key: a.0.to_string(),
                    value: a.1,
                })
                .collect(),
            response_type: self
                .response_type
                .iter()
                .map(|a| ItemKeyValueRate {
                    key: a.0.to_string(),
                    value: a.1,
                    rate: a.2,
                })
                .collect(),
            answer_result: self
                .answer_result
                .iter()
                .map(|a| ItemKeyValue {
                    key: a.0.to_string(),
                    value: a.1,
                })
                .collect(),
            additional_result: self
                .additional_result
                .iter()
                .map(|a| ItemKeyValue {
                    key: a.0.to_string(),
                    value: a.1,
                })
                .collect(),
            authority_result: self
                .authority_result
                .iter()
                .map(|a| ItemKeyValue {
                    key: a.0.to_string(),
                    value: a.1,
                })
                .collect(),
        }
    }
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

#[derive(Serialize, Deserialize, Debug)]
struct CombinedResult {
    basic: BasicStatsSerializable,
    extension: ExtensionStatsSerializable,
}

impl ReportType {
    fn formatted_data(report: &RunnerReport) -> CombinedResult {
        CombinedResult {
            basic: BasicStats::new(report).to_serializable(),
            extension: ExtensionStats::new(report).to_serializable(),
        }
    }

    fn basic(report: &RunnerReport) {
        let formatted = ReportType::formatted_data(report);
        let extension_info = formatted.extension;
        let basic_info = formatted.basic;

        let query: Vec<_> = extension_info
            .query_type
            .iter()
            .map(|a| format!("{}={}", a.key, a.value))
            .collect();

        let response_code: String = basic_info
            .response_code
            .iter()
            .map(|a| format!("{}={}", a.key, a.value))
            .collect::<Vec<String>>()
            .join(",");

        let out_put = format!(
            "------------   Report   --------------
      Total Cost: {:?}
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
            basic_info.duration,
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
        println!("{}", out_put);
    }
    fn yaml(report: &RunnerReport, output: String) {
        let formatted = ReportType::formatted_data(report);
        match serde_yaml::to_string(&formatted) {
            Err(err) => error!("yaml convert fail: {}", err.to_string()),
            Ok(v) => {
                let mut buffer = File::create(output).expect("create file error");
                if let Err(e) = buffer.write_all(v.as_bytes()) {
                    error!("{}", e.to_string())
                }
            }
        }
    }
    fn json(report: &RunnerReport, output: String) {
        let formatted = ReportType::formatted_data(report);
        match serde_json::to_string_pretty(&formatted) {
            Err(err) => error!("json convert fail: {}", err.to_string()),
            Ok(v) => {
                let mut buffer = File::create(output).expect("create file error");
                if let Err(e) = buffer.write_all(v.as_bytes()) {
                    error!("{}", e.to_string())
                }
            }
        }
    }
}

impl ReportOutput for ReportType {
    fn format(&self, report: &RunnerReport, output: String) {
        match self {
            ReportType::Basic => ReportType::basic(report),
            ReportType::YAML => ReportType::yaml(report, output),
            ReportType::JSON => ReportType::json(report, output),
        }
    }
}

fn format_result(result_map: &HashMap<u16, u64>) -> Vec<(RecordType, u64)> {
    let mut to_tuple: Vec<_> = result_map.iter().collect();
    to_tuple.sort_by_key(|a| a.0);
    to_tuple
        .iter()
        .map(|a| {
            let query_type = RecordType::from(*a.0);
            (query_type, *a.1)
        })
        .collect::<Vec<(RecordType, u64)>>()
}

fn format_code_result(result_map: &HashMap<u8, u64>) -> Vec<(ResponseCode, u64)> {
    let mut to_tuple: Vec<_> = result_map.iter().collect();
    to_tuple.sort_by_key(|a| a.0);
    to_tuple
        .iter()
        .map(|a| {
            let query_type = trust_dns_client::op::ResponseCode::from(0, *a.0);
            (query_type, *a.1)
        })
        .collect::<Vec<(ResponseCode, u64)>>()
}
