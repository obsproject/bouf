#![allow(dead_code)]

use std::path::PathBuf;

mod models;
mod utils;

use crate::utils::sign::Signer;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(about, long_about = None)]
struct Args {
    // Will use "UPDATER_PRIVATE_KEY" env var if not set
    #[arg(short, long, value_name = "Private key PEM file")]
    private_key: Option<PathBuf>,

    #[arg(short, long, value_name = "Files to sign")]
    files: Vec<PathBuf>,
}

fn main() {
    let args: Args = Args::parse();

    let mut signer = Signer::init(args.private_key.as_ref());

    for f in args.files {
        println!("Signing \"{}\"", f.display());
        signer.sign_file(&f).expect("Failed to sign file!");
    }
}
