use std::fmt;
use std::fmt::Formatter;
use std::net::IpAddr;
use std::str::FromStr;
use structopt::StructOpt;
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
// #[derive(Debug, Clone, Default)]
// pub struct DomainTypeVec(pub(crate) Vec<RecordType>);
//
// impl DomainTypeVec {
//     pub fn size(&self) -> usize {
//         self.0.len()
//     }
// }

// impl Display for DomainTypeVec {
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         let mut dtype_vec = vec![];
//         for x in &self.0 {
//             dtype_vec.push(x.to_string());
//         }
//         write!(f, "{}", dtype_vec.join(","))
//     }
// }

#[derive(Debug, Clone, StructOpt)]
#[structopt(name = "snd", about = "a dns traffic generator")]
pub struct Argument {
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
        short = "f",
        long = "file",
        default_value = "",
        help = "the dns query file"
    )]
    pub file: String,

    #[structopt(long = "file-loop", help = "read dns query file in loop mode")]
    pub fileloop: bool,

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
        default_value = "A",
        help = "dns query type"
    )]
    pub qty: String,

    #[structopt(
        long = "timeout",
        default_value = "5",
        help = "timeout for wait the packet arrive"
    )]
    pub timeout: usize,

    #[structopt(
        long = "packet-id",
        default_value = "0",
        help = "set to zero will random select a packet id"
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
    // set the default max payload to 1232
    // https://dnsflagday.net/2020/
    pub disable_edns: bool,
    #[structopt(
        long = "edns-size",
        default_value = "1232",
        help = "edns size for dns packet and receive buffer"
    )]
    pub edns_size: u16,

    #[structopt(long = "debug", help = "enable debug mode")]
    pub debug: bool,

    #[structopt(long = "source-ip",
        parse(try_from_str = parse_ip),
        default_value = "0.0.0.0",
        help = "set the source ip address")
    ]
    pub source: IpAddr,

    #[structopt(
        long = "check-all-message",
        help = "default only check response header"
    )]
    pub check_all_message: bool,
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
     Enable DNSSEC: {},
 Check All Message: {}\n",
            env!("CARGO_PKG_VERSION"),
            {
                if self.file.is_empty() {
                    self.domain.as_str()
                } else {
                    self.file.as_str()
                }
            },
            {
                if self.file.is_empty() {
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
            self.check_all_message
        )
    }
}
