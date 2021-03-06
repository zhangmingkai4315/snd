use crate::utils::utils::cpu_mode_to_cpu_cores;
use num_cpus;
use std::fmt;
use std::fmt::Formatter;
use std::fs;
use std::net::IpAddr;
use std::str::FromStr;
use structopt::StructOpt;
use trust_dns_client::proto::rr::RecordType;
use trust_dns_client::rr::Name;
use validator::validate_ip;

#[derive(Debug, Clone)]
pub enum Protocol {
    UDP,
    TCP,
    DOH,
    DOT,
}
impl Default for Protocol {
    fn default() -> Self {
        Protocol::UDP
    }
}

impl FromStr for Protocol {
    type Err = String;
    fn from_str(protocol: &str) -> Result<Self, Self::Err> {
        match protocol.to_uppercase().as_str() {
            "UDP" => Ok(Protocol::UDP),
            "TCP" => Ok(Protocol::TCP),
            "DOH" => Ok(Protocol::DOH),
            "DOT" => Ok(Protocol::DOT),
            _ => Err(format!("protocol {} not valid", protocol)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum DoHMethod {
    Get,
    Post,
}

impl Default for DoHMethod {
    fn default() -> Self {
        DoHMethod::Get
    }
}

impl FromStr for DoHMethod {
    type Err = String;
    fn from_str(method: &str) -> Result<Self, Self::Err> {
        match method.to_uppercase().as_str() {
            "POST" => Ok(DoHMethod::Post),
            "GET" => Ok(DoHMethod::Get),
            _ => Err(format!("doh method {} not valid", method)),
        }
    }
}
fn parse_ip(value: &str) -> Result<IpAddr, String> {
    match IpAddr::from_str(value) {
        Ok(v) => Ok(v),
        _ => Err(format!("source ip address {} not correct!", value)),
    }
}
fn parse_server(value: &str) -> Result<String, String> {
    let mut is_ip = false;
    let mut is_domain = false;
    if validate_ip(value) == true {
        is_ip = true;
    }
    if let Ok(_) = Name::from_str(value) {
        is_domain = true;
    }
    if (is_ip || is_domain) == false {
        return Err(format!("{} is not ip or domain", value));
    }
    Ok(value.to_owned())
}

#[derive(Debug, Clone, StructOpt)]
#[structopt(
    name = "snd",
    about = "a dns traffic generator",
    help = "snd 0.1.0
a dns traffic generator

USAGE:
snd [OPTIONS] [FLAGS]

OPTIONS:
    -s, --server <server>                          the dns server for benchmark [default: 8.8.8.8]
    -p, --port <port>                              the dns server port number [default: 53]
    -d, --domain <domain>                          domain name for dns query [default: example.com]
    -t, --type <qty>                               dns query type [default: A]
    -T, --time <time>                              how long it will send until stop [default: 0]
    -q, --qps <qps>                                dns query per second [default: 10]
    -m, --max <max>                                max dns packets will be send [default: 100]
    -c, --client <client>                          concurrent clients numbers, set to 0 will replace with the number of cpu cores [default: 0]
    -f, --file <file>                              the dns query file, default using -d for single domain query [default: \"\"]
    -o, --output <file>                            format output report to stdout, .json or .yaml file [default: \"stdout\"]
    -I, --interval <second>                        output result interval for dns benchmark [default: 0]
        --edns-size <edns-size>                    set opt max EDNS buffer size [default: 1232]
        --protocol <protocol>                      the packet protocol for send dns request [default: UDP]
                                                   support protocols [UDP, TCP]
        --source-ip <source>                       set the source ip address [default: 0.0.0.0]
        --timeout <timeout>                        timeout for wait the packet arrive [default: 5]
        --bind-cpu <mode>                          bind worker to cpu [default: random]
                                                   option value [\"random\", \"all\", \"0,1,2,3\", \"0,3\"]

FLAGS:
        --debug                enable debug mode
        --disable-edns         disable EDNS
        --disable-rd           RD (recursion desired) bit in the query
        --enable-cd            CD (checking disabled) bit in the query
        --enable-dnssec        enable dnssec
HELP:
    -h, --help                 Prints help information
VERSION:
    -V, --version              Prints version information
"
)]

//         --doh-server <doh-server>                  doh server based RFC8484 [default: https://dns.alidns.com/dns-query]
//         --doh-server-method <doh-server-method>    doh http method[GET/POST] [default: GET]

pub struct Argument {
    #[structopt(
        short = "s",
        long = "server",
        default_value = "8.8.8.8",
        parse(try_from_str = parse_server),
    )]
    pub server: String,
    #[structopt(
        short = "p",
        long = "port",
        default_value = "53",
        // validator = is_port,
    )]
    pub port: u16,

    #[structopt(short = "f", long = "file", default_value = "")]
    pub file: String,

    #[structopt(long = "file-loop")]
    pub fileloop: bool,

    #[structopt(long = "protocol", default_value = "UDP")]
    pub protocol: Protocol,

    #[structopt(short = "q", long = "qps", default_value = "10")]
    pub qps: usize,
    #[structopt(short = "m", long = "max", default_value = "0")]
    pub max: usize,

    #[structopt(short = "T", long = "time", default_value = "0")]
    pub until_stop: usize,

    #[structopt(short = "c", long = "client", default_value = "0")]
    pub client: usize,

    #[structopt(short = "d", long = "domain", default_value = "example.com")]
    pub domain: String,
    #[structopt(short = "t", long = "type", default_value = "A")]
    pub qty: String,

    #[structopt(long = "timeout", default_value = "5")]
    pub timeout: usize,

    #[structopt(long = "packet-id", default_value = "0")]
    pub packet_id: u16,

    #[structopt(long = "doh-server-method", default_value = "GET")]
    pub doh_server_method: DoHMethod,

    #[structopt(
        long = "doh-server",
        default_value = "https://dns.alidns.com/dns-query"
    )]
    pub doh_server: String,

    #[structopt(long = "disable-rd")]
    pub disable_rd: bool,
    #[structopt(long = "enable-cd")]
    pub enable_cd: bool,

    #[structopt(long = "enable-dnssec")]
    pub enable_dnssec: bool,

    #[structopt(long = "disable-edns")]
    // set the default max payload to 1232
    // https://dnsflagday.net/2020/
    pub disable_edns: bool,
    #[structopt(long = "edns-size", default_value = "1232")]
    pub edns_size: u16,

    #[structopt(long = "debug")]
    pub debug: bool,

    #[structopt(long = "source-ip",
        parse(try_from_str = parse_ip),
        default_value = "0.0.0.0")
    ]
    pub source: IpAddr,

    #[structopt(long = "bind-cpu", default_value = "random")]
    pub bind_cpu: String,

    #[structopt(short = "o", long = "output", default_value = "stdout")]
    pub output: String,

    #[structopt(short = "I", long = "interval", default_value = "0")]
    pub output_interval: usize,
}

impl Argument {
    pub fn validate(&mut self) -> Result<(), String> {
        if self.file.is_empty() {
            if let Err(e) = Name::from_str(self.domain.clone().as_str()) {
                return Err(format!(
                    "domain name {} parse fail: {}",
                    self.domain,
                    e.to_string()
                ));
            }
            if let Err(e) = RecordType::from_str(self.qty.as_str()) {
                return Err(format!(
                    "query type {} parse fail: {}",
                    self.qty,
                    e.to_string()
                ));
            }
        } else {
            if fs::metadata(self.file.clone()).is_err() {
                return Err(format!("open file {} error", self.file));
            }
        }
        if self.domain.is_empty() && self.file.is_empty() {
            return Err(format!("must set domain or query file"));
        }
        if self.until_stop == 0 && self.max == 0 {
            return Err(format!("must set max query [-m] or time limit [-T]"));
        }
        if self.client == 0 {
            self.client = num_cpus::get();
        }
        if let Err(e) = cpu_mode_to_cpu_cores(self.bind_cpu.clone()) {
            return Err(e.to_string());
        }

        if !(self.output.to_lowercase().ends_with(".json")
            || self.output.to_lowercase().ends_with(".yaml")
            || self.output.to_lowercase() == "stdout".to_string())
        {
            return Err("output result should be setting with -o example.json or -o example.yaml file or -o stdout".to_string());
        }

        Ok(())
    }
}

impl Default for Argument {
    fn default() -> Self {
        Argument {
            server: "8.8.8.8".to_string(),
            port: 53,
            file: "".to_string(),
            fileloop: false,
            protocol: Default::default(),
            qps: 10,
            max: 100,
            client: 1,
            domain: "google.com".to_string(),
            qty: "NS".to_string(),
            timeout: 5,
            until_stop: 0,
            packet_id: 0,
            doh_server_method: Default::default(),
            doh_server: "".to_string(),
            disable_rd: false,
            enable_cd: false,
            enable_dnssec: false,
            disable_edns: false,
            edns_size: 0,
            debug: false,
            source: IpAddr::from_str("0.0.0.0").unwrap(),
            bind_cpu: "random".to_string(),
            output: "stdout".to_string(),
            output_interval: 0,
        }
    }
}

impl fmt::Display for Argument {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "
DNS Traffic Generator <SND>
Version: {}
------------ Basic Setting -----------
            Domain: {}
        Query Type: {}
            Server: {}
Transport Protocol: {:?}
     Client Number: {}
  Query Per Second: {}
 Max Packet Number: {},
   Time Until Stop: {},
------------ Advance Setting ---------
         Packet ID: {}
    Turn On RD Bit: {},
    Turn On CD Bit: {},
       Enable EDNS: {},
         EDNS Size: {},
     Enable DNSSEC: {},
     Bind CPU Mode: {},
            Output: {},
          Interval: {:?}\n",
            env!("CARGO_PKG_VERSION"),
            {
                if self.file.is_empty() {
                    self.domain.as_str()
                } else {
                    self.file.as_str()
                }
            },
            {
                if self.file.is_empty() == false {
                    "from query file"
                } else {
                    self.qty.as_str()
                }
            },
            {
                match self.protocol {
                    Protocol::DOH => {
                        format!("{}[{:?}]", self.doh_server.clone(), self.doh_server_method)
                    }
                    _ => format!("{}/{}", self.server.clone(), self.port),
                }
            },
            self.protocol,
            self.client,
            {
                match self.qps {
                    0 => "unlimited".to_owned(),
                    _ => format!("{}", self.qps),
                }
            },
            self.max,
            {
                if self.until_stop == 0 {
                    "unlimited".to_owned()
                } else {
                    format!("{}s", self.until_stop)
                }
            },
            {
                match self.packet_id {
                    0 => "random".to_owned(),
                    _ => format!("{}", self.packet_id),
                }
            },
            !self.disable_rd,
            self.enable_cd,
            self.disable_edns,
            self.edns_size,
            self.enable_dnssec,
            self.bind_cpu,
            self.output,
            std::time::Duration::from_secs(self.output_interval as u64),
        )
    }
}
