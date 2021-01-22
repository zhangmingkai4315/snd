extern crate base64;
extern crate chrono;
extern crate governor;
extern crate leaky_bucket;
extern crate net2;
extern crate nonzero_ext;
extern crate rand;
extern crate reqwest;
extern crate structopt;
extern crate trust_dns_client;
extern crate validator;
#[macro_use]
extern crate log;
extern crate crossbeam_channel;
extern crate rustls;
extern crate webpki;
extern crate webpki_roots;
// extern crate stream_histogram;

mod arguments;
mod cache;
mod histogram;
mod report;
mod runner;
mod workers;

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
