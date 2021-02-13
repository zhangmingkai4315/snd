extern crate structopt;
#[macro_use]
extern crate log;

use env_logger::{Builder, Target};
use log::LevelFilter;
use structopt::StructOpt;

use lib::runner::Runner;
use lib::utils::Argument;

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
