extern crate futures;
extern crate leaky_bucket;
extern crate tokio;

extern crate governor;
extern crate nonzero_ext;
extern crate rand;
extern crate structopt;
extern crate trust_dns_client;
#[macro_use]
extern crate log;
extern crate crossbeam_channel;

mod arguments;
mod cache;
mod report;
mod runner;

use arguments::Argument;
use env_logger::{Builder, Target};
use log::LevelFilter;
use runner::Runner;
use std::error::Error;
use structopt::StructOpt;

fn main() -> Result<(), Box<dyn Error>> {
    let mut builder = Builder::from_default_env();
    builder
        .target(Target::Stdout)
        .filter_level(LevelFilter::Info);
    builder.init();
    let arg = Argument::from_args();
    println!("{}", arg);
    Runner::new(arg);
    Ok(())
}
