extern crate structopt;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "snd", about = "a dns traffic generator")]
struct Argument {
    #[structopt(long = "debug")]
    debug: bool,

    #[structopt(short = "p", default_value = "53")]
    port: u16,

    #[structopt(short = "s", default_value = "8.8.8.8")]
    server: String,

    #[structopt(long = "qps", default_value = "10")]
    qps: usize,
    #[structopt(long = "max", default_value = "100")]
    max: usize,
    #[structopt(long = "timeout", default_value = "10")]
    timeout: usize,

    #[structopt(short = "d", long = "domain", default_value = "www.google.com")]
    domain: String,
}

fn main() {
    let arg = Argument::from_args();
    println!("{:?}", arg);
}
