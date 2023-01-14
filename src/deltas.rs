#![allow(dead_code)]

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

mod models;
mod steps;
mod utils;

use crate::models::config::Config;
use steps::generate::Generator;

#[derive(Parser, Debug)]
#[clap(about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser, value_name = "Config file")]
    pub config: PathBuf,
}

fn main() -> Result<()> {
    let args: Args = Args::parse();
    println!("[+] Loading config...");
    let mut conf = Config::from_file(&args.config)?;
    conf.validate(true)?;
    let mut gen = Generator::init(&conf, false);
    println!("[+] Running generator...");
    gen.create_patches().context("[!] Creating delta patches failed!")?;

    Ok(())
}
