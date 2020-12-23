use chrono::DateTime;
use chrono::Local;
use std::collections::HashMap;
use trust_dns_client::op::Message;
use trust_dns_client::rr::RecordType;

#[derive(Default, Clone)]
pub struct QueryStatusStore {
    total: usize,
    query_type: HashMap<u16, usize>,
    answer_type: HashMap<u16, usize>,
    authority_type: HashMap<u16, usize>,
    additional_type: HashMap<u16, usize>,
    reply_code: HashMap<u16, usize>,
}

impl QueryStatusStore {
    pub fn update_query(&mut self, query_type: u16) {
        self.total = self.total + 1;
        match self.query_type.get(&query_type) {
            Some(v) => self.query_type.insert(query_type, v + 1),
            _ => self.query_type.insert(query_type, 1),
        };
    }

    pub fn update_response(&mut self, message: &Message) {
        self.total = self.total + 1;
        let query_type = u16::from(message.queries()[0].query_type());
        match self.query_type.get(&query_type) {
            Some(v) => self.query_type.insert(query_type, v + 1),
            _ => self.query_type.insert(query_type, 1),
        };

        for answer in message.answers() {
            let query_type = u16::from(answer.record_type());
            match self.answer_type.get(&query_type) {
                Some(v) => self.answer_type.insert(query_type, v + 1),
                _ => self.answer_type.insert(query_type, 1),
            };
        }

        for answer in message.additionals() {
            let query_type = u16::from(answer.record_type());
            match self.additional_type.get(&query_type) {
                Some(v) => self.additional_type.insert(query_type, v + 1),
                _ => self.additional_type.insert(query_type, 1),
            };
        }

        for answer in message.name_servers() {
            let query_type = u16::from(answer.record_type());
            match self.authority_type.get(&query_type) {
                Some(v) => self.authority_type.insert(query_type, v + 1),
                _ => self.authority_type.insert(query_type, 1),
            };
        }

        let r_code = u16::from(message.response_code());
        match self.reply_code.get(&r_code) {
            Some(v) => self.reply_code.insert(r_code, v + 1),
            _ => self.reply_code.insert(r_code, 1),
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
                let query_type = RecordType::from(*a.0).to_string();
                let rate: f64 = {
                    if let Some(query) =
                        report.producer_report.as_ref().unwrap().query_type.get(a.0)
                    {
                        *a.1 as f64 * 100.0 / *query as f64
                    } else {
                        0.0
                    }
                };
                format!("{} = {}({:.2}%)", query_type, a.1, rate)
            })
            .collect();

        let answer_result: String = format_result(&report
            .consumer_report
            .as_ref()
            .unwrap()
            .answer_type).join(",");

        let additional_result: String = format_result(&report
            .consumer_report
            .as_ref()
            .unwrap()
            .additional_type).join(",");

        let authority_result: String = format_result(&report
            .consumer_report
            .as_ref()
            .unwrap()
            .authority_type).join(",");

        let start_time: DateTime<Local> = report.start.into();
        let end_time: DateTime<Local> = report.end.unwrap().into();
        format!(
            "------------ Report -----------
>> Total Cost      : {:?} (+5s time wait)
   Start Time      : {}
   End Time        : {}

>> Total Query     : {}
   Question Type   : {}

>> Total Response  : {}
   Question Type   : {}
   Answer Type     : {}
   Authority Type  : {}
   Additional Type : {}

Success Rate       : {:.2}%\n",
            report.end.unwrap().duration_since(report.start).unwrap(),
            start_time.format("%+"),
            end_time.format("%+"),
            report.producer_report.as_ref().unwrap().total,
            query.join(","),
            report.consumer_report.as_ref().unwrap().total,
            response.join(","),
            answer_result,
            authority_result,
            additional_result,
            report.consumer_report.as_ref().unwrap().total as f64 * 100.0
                / report.producer_report.as_ref().unwrap().total as f64,
        )
    }
    // fn color(report: &RunnerReport) -> String {
    //     format!("colorful output")
    // }
}

impl ReportOutput for ReportType {
    fn format(&self, report: &RunnerReport) -> String {
        match self {
            ReportType::Basic => ReportType::basic(report),
            // ReportType::Color => ReportType::color(report),
            _ => unimplemented!(),
        }
    }
}


fn format_result(result_map: &HashMap<u16,usize>)->Vec<String>{
    let mut to_tuple: Vec<_> = result_map.iter().collect();
    to_tuple.sort_by_key(|a| a.0);
    to_tuple
        .iter()
        .map(|a| {
            let query_type = RecordType::from(*a.0).to_string();
            format!("{} = {}", query_type, a.1)
        })
        .collect()
}