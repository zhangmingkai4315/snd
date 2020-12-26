use std::fmt;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use structopt::StructOpt;
use trust_dns_client::rr::{Name, RecordType};
use validator::validate_ip;

#[derive(Debug, Clone)]
pub enum Protocol {
    UDP,
    TCP,
    DOH,
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
#[derive(Debug, Clone, Default)]
pub struct DomainTypeVec(pub(crate) Vec<RecordType>);

impl DomainTypeVec {
    pub fn size(&self) -> usize {
        self.0.len()
    }
}

impl Display for DomainTypeVec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut dtype_vec = vec![];
        for x in &self.0 {
            dtype_vec.push(x.to_string());
        }
        write!(f, "{}", dtype_vec.join(","))
    }
}

fn parse_domain_type(value: &str) -> Result<DomainTypeVec, String> {
    let mut type_vec = vec![];
    for x in value.to_uppercase().split(",") {
        match RecordType::from_str(x) {
            Ok(v) => type_vec.push(v),
            _ => return Err(format!("unknown type: {}", x)),
        }
    }
    Ok(DomainTypeVec(type_vec))
}

#[derive(Debug, Clone, Default, StructOpt)]
#[structopt(name = "snd", about = "a dns traffic generator")]
pub(crate) struct Argument {
    #[structopt(
        short = "s",
        long = "server",
        default_value = "8.8.8.8",
        help = "the dns server for benchmark",
        parse(try_from_str = parse_server),
    )]
    pub server: String,
    #[structopt(
        short = "p",
        long = "port",
        default_value = "53",
        help = "the dns server port number",
        // validator = is_port,
    )]
    pub port: u16,
    #[structopt(
        long = "protocol",
        default_value = "UDP",
        help = "the packet protocol for send dns request"
    )]
    pub protocol: Protocol,

    #[structopt(
        short = "q",
        long = "qps",
        default_value = "10",
        help = "dns query per second"
    )]
    pub qps: usize,
    #[structopt(
        short = "m",
        long = "max",
        default_value = "100",
        help = "max dns packets will be send"
    )]
    pub max: usize,

    #[structopt(
        short = "c",
        long = "client",
        default_value = "10",
        help = "concurrent dns query clients"
    )]
    pub client: usize,

    #[structopt(
        short = "d",
        long = "domain",
        default_value = "example.com",
        help = "domain name for dns query"
    )]
    pub domain: String,
    #[structopt(
        short = "t",
        long = "type",
        default_value = "A,SOA",
        help = "dns query type [multi types supported]",
        parse(try_from_str = parse_domain_type)
    )]
    pub qty: DomainTypeVec,

    #[structopt(
        long = "timeout",
        default_value = "5",
        help = "timeout for wait the packet arrive"
    )]
    pub timeout: usize,

    #[structopt(
        long = "packet-id",
        default_value = "0",
        help = "fix the packet id or set to zero will random select a number"
    )]
    pub packet_id: u16,

    #[structopt(
        long = "doh-server-method",
        default_value = "GET",
        help = "doh http method[GET/POST]"
    )]
    pub doh_server_method: DoHMethod,

    #[structopt(
        long = "doh-server",
        default_value = "https://dns.alidns.com/dns-query",
        help = "doh server based RFC8484"
    )]
    pub doh_server: String,

    #[structopt(long = "disable-rd", help = "RD (recursion desired) bit in the query")]
    pub disable_rd: bool,
    #[structopt(long = "enable-cd", help = "CD (checking disabled) bit in the query")]
    pub enable_cd: bool,

    #[structopt(long = "enable-dnssec", help = "enable dnssec")]
    pub enable_dnssec: bool,

    #[structopt(long = "disable-edns", help = "disable edns")]
    pub disable_edns: bool,
    #[structopt(
        long = "edns-size",
        default_value = "1232",
        help = "edns size for dns packet and receive buffer"
    )]
    pub edns_size: u16,

    #[structopt(long = "debug", help = "enable debug mode")]
    pub debug: bool,
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
------------ Advance Setting ---------
         Packet ID: {}
    Turn On RD Bit: {},
    Turn On CD Bit: {},
       Enable EDNS: {},
         EDNS Size: {},
     Enable DNSSEC: {}\n",
            env!("CARGO_PKG_VERSION"),
            self.domain,
            self.qty,
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
                match self.packet_id {
                    0 => "random".to_owned(),
                    _ => format!("{}", self.packet_id),
                }
            },
            !self.disable_rd,
            self.enable_cd,
            self.disable_edns,
            self.edns_size,
            self.enable_dnssec
        )
    }
}
