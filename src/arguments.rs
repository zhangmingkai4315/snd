use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "snd", about = "a dns traffic generator")]
pub(crate) struct Argument {
    #[structopt(long = "debug")]
    pub debug: bool,

    #[structopt(short = "p", default_value = "53")]
    pub port: u16,

    #[structopt(short = "s", default_value = "8.8.8.8")]
    pub server: String,

    #[structopt(long = "qps", default_value = "10")]
    pub qps: usize,
    #[structopt(long = "max", default_value = "100")]
    pub max: usize,
    #[structopt(long = "timeout", default_value = "10")]
    pub timeout: usize,

    #[structopt(short = "c", long = "client", default_value = "10")]
    pub client: usize,

    #[structopt(short = "d", long = "domain", default_value = "www.google.com")]
    pub domain: String,

    #[structopt(short = "t", long = "type", default_value = "A,NS")]
    pub qty: String,

    #[structopt(long = "id_random")]
    pub id_random: bool,
}

impl Default for Argument {
    fn default() -> Argument {
        Argument {
            debug: false,
            port: 53,
            server: "8.8.8.8".to_string(),
            qps: 10,
            max: 100,
            timeout: 5,
            client: 5,
            domain: "www.google.com".to_string(),
            qty: "A".to_string(),
            id_random: false,
        }
    }
}
