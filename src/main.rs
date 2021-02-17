extern crate log;
extern crate structopt;

use env_logger::{Builder, Target};
use lib::runner::Runner;
use lib::utils::Argument;
use log::LevelFilter;
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
    match Runner::new(arg) {
        Ok(mut v) => v.run(),
        Err(e) => {
            println!("start runner error: {:?}", e);
        }
    }
}
