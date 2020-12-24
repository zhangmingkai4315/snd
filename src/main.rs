extern crate chrono;
extern crate futures;
extern crate governor;
extern crate leaky_bucket;
extern crate nonzero_ext;
extern crate rand;
extern crate structopt;
extern crate tokio;
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
    let arg = Argument::from_args();
    println!("{}", arg);
    let mut builder = Builder::from_default_env();
    builder.target(Target::Stdout).filter_level({
        match arg.debug {
            true => LevelFilter::Debug,
            false => LevelFilter::Info,
        }
    });
    builder.init();
    Runner::new(arg);
    Ok(())
}
