use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;

mod steps;
mod utils;

#[derive(Parser, Debug)]
#[clap(about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser, value_name = "Config file")]
    config: Option<PathBuf>,

    #[clap(long, value_parser, value_name = "new build")]
    new: PathBuf,
    #[clap(long, value_parser, value_name = "old builds")]
    old: PathBuf,
    #[clap(long, value_parser, value_name = "output dir")]
    out: PathBuf,
}

fn main() {
    let args = Args::parse();

    let path = fs::canonicalize(args.new).unwrap();
    let old_path = fs::canonicalize(args.old).unwrap();
    let out_path = fs::canonicalize(&args.out).unwrap_or(args.out);

    steps::generate::create_patches(path.as_path(), old_path.as_path(), out_path.as_path())
}
