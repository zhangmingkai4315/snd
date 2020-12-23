use std::fmt;
use std::fmt::Formatter;
use structopt::StructOpt;

#[derive(Debug, Clone, Default, StructOpt)]
#[structopt(name = "snd", about = "a dns traffic generator")]
pub(crate) struct Argument {
    #[structopt(long = "debug")]
    pub debug: bool,

    #[structopt(short = "p", long = "port", default_value = "53")]
    pub port: u16,

    #[structopt(short = "s", long = "server", default_value = "8.8.8.8")]
    pub server: String,

    #[structopt(short = "q", long = "qps", default_value = "10")]
    pub qps: usize,
    #[structopt(short = "m", long = "max", default_value = "100")]
    pub max: usize,

    #[structopt(short = "c", long = "client", default_value = "10")]
    pub client: usize,

    #[structopt(short = "d", long = "domain", default_value = "www.google.com")]
    pub domain: String,
    #[structopt(short = "t", long = "type", default_value = "A,NS")]
    pub qty: String,
    #[structopt(long = "timeout", default_value = "10")]
    pub timeout: usize,
    #[structopt(long = "id-random")]
    pub id_random: bool,
    #[structopt(long = "no-rd")]
    pub nord: bool,
}

impl fmt::Display for Argument {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "
DNS Traffic Generator <SND>
Version: {}
------------ Basic Setting -----------
Domain            : {}
Query Type        : {}
Server            : {}/{}
Client Number     : {}
Query Per Second  : {}
Max Packet Number : {},
------------ Advance Setting ---------
Random Port       : {}
Turn Off RD Bit   : {}\n",
            env!("CARGO_PKG_VERSION"),
            self.domain,
            self.qty,
            self.server,
            self.port,
            self.client,
            {
                match self.qps {
                    0 => "unlimited".to_owned(),
                    _ => format!("{}", self.qps),
                }
            },
            self.max,
            self.id_random,
            self.nord,
        )
    }
}
