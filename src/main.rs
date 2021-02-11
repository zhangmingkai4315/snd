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
extern crate num_cpus;
extern crate rustls;
extern crate serde_json;
extern crate serde_yaml;
extern crate toml;
extern crate webpki;
extern crate webpki_roots;

// extern crate stream_histogram;

mod arguments;
mod runner;
mod workers;

use arguments::Argument;
use env_logger::{Builder, Target};
use log::LevelFilter;
use runner::Runner;
use structopt::StructOpt;

fn main() {
    let mut arg: Argument = Argument::from_args();
    if let Err(err) = arg.validate() {
        println!("validate error: {}", err.as_str());
        return;
    }
    println!("{}", arg);
    let mut builder = Builder::from_default_env();
    builder.target(Target::Stdout).filter_level({
        match arg.debug {
            true => LevelFilter::Debug,
            false => LevelFilter::Info,
        }
    });
    builder.init();
    let mut runner = Runner::new(arg);
    runner.run();
}
