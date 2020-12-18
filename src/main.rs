extern crate governor;
extern crate structopt;
extern crate trust_dns_client;

mod arguments;
mod cache;
mod runner;

use arguments::Argument;
use runner::Runner;
use structopt::StructOpt;

fn main() {
    let arg = Argument::from_args();
    Runner::new(arg);
}
