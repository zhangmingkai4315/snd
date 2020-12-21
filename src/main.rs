extern crate governor;
extern crate rand;
extern crate structopt;
extern crate trust_dns_client;
#[macro_use]
extern crate log;

mod arguments;
mod cache;
mod runner;

use arguments::Argument;
use env_logger::{Builder, Target};
use log::LevelFilter;
use runner::Runner;
use structopt::StructOpt;

fn main() {
    let mut builder = Builder::from_default_env();
    builder
        .target(Target::Stdout)
        .filter_level(LevelFilter::Info);
    builder.init();
    let mut arg = Argument::from_args();
    println!("{}", arg);
    Runner::new(&mut arg);
}
