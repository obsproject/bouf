#![allow(dead_code)]

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use log::info;

mod models;
mod steps;
mod utils;

use models::config::Config;
use steps::generate::Generator;
use utils::logging::init_logger;

#[derive(Parser, Debug)]
#[command(about, long_about = None)]
struct Args {
    #[arg(short, long, value_name = "Config file")]
    pub config: PathBuf,
}

fn main() -> Result<()> {
    let args: Args = Args::parse();
    info!("Loading config...");
    let mut conf = Config::from_file(&args.config)?;
    init_logger(conf.general.log_level.as_str());

    conf.validate(true)?;
    let mut gen = Generator::init(&conf, false);
    info!("Running generator...");
    gen.create_patches().context("Creating delta patches failed!")?;

    Ok(())
}
