#![allow(dead_code)]

use std::path::PathBuf;

use crate::utils::sign::Signer;
use clap::Parser;

mod models;
mod utils;

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

    let mut signer = Signer::init();
    if let Some(key_file) = &args.private_key {
        signer = signer.with_keyfile(key_file);
    }

    for f in args.files {
        println!("Signing \"{}\"", f.display());
        signer.sign_file(&f).expect("Failed to sign file!");
    }
}
