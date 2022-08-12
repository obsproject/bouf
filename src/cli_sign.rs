#![allow(dead_code)]

use std::path::{Path, PathBuf};

use clap::Parser;

mod utils;
use crate::utils::config::Config;

#[derive(Parser, Debug)]
#[clap(about, long_about = None)]
struct Args {
    // Will use "UPDATER_PRIVATE_KEY" env var if not set
    #[clap(short, long, value_parser, value_name = "Private key PEM file")]
    private_key: Option<PathBuf>,

    #[clap(value_parser)]
    files: Vec<PathBuf>,
}

fn main() {
    let args: Args = Args::parse();

    let key = utils::sign::load_key(args.private_key).expect("Failed to load private key!");

    for f in args.files {
        println!("Signing \"{}\"", f.display());
        utils::sign::sign_file(&key, &f).expect("Failed to sign file!");
    }
}
