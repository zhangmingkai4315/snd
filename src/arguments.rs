use std::fmt;
use std::fmt::Formatter;
use std::str::FromStr;
use structopt::StructOpt;

#[derive(Debug, Clone)]
pub enum Protocol {
    UDP,
    TCP,
}

impl Default for Protocol {
    fn default() -> Self {
        Protocol::UDP
    }
}

impl FromStr for Protocol {
    type Err = String;
    fn from_str(protocol: &str) -> Result<Self, Self::Err> {
        match protocol {
            "udp" | "UDP" => Ok(Protocol::UDP),
            "tcp" | "TCP" => Ok(Protocol::TCP),
            _ => Err(format!("protocol {} not valid", protocol)),
        }
    }
}

#[derive(Debug, Clone, Default, StructOpt)]
#[structopt(name = "snd", about = "a dns traffic generator")]
pub(crate) struct Argument {
    #[structopt(
        short = "s",
        long = "server",
        default_value = "8.8.8.8",
        help = "the dns server for benchmark"
    )]
    pub server: String,
    #[structopt(
        short = "p",
        long = "port",
        default_value = "53",
        help = "the dns server port number"
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
        help = "dns query type [multi types supported]"
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
        help = "fix the packet id or set to zero will random select a number"
    )]
    pub packet_id: u16,

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
            Server: {}/{}
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
            self.server,
            self.port,
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
