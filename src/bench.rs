#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::utils::hash::FileInfo;
use anyhow::{bail, Result};
use clap::Parser;

mod models;
mod utils;

#[derive(Parser, Debug)]
#[clap(about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser, value_name = "Old input file")]
    pub old_file: PathBuf,
    #[clap(short, long, value_parser, value_name = "New input file")]
    pub new_file: PathBuf,
}

fn main() -> Result<()> {
    let args: Args = Args::parse();
    if !args.old_file.exists() {
        bail!("Old file does not exist!")
    }
    if !args.new_file.exists() {
        bail!("New file does not exist!")
    }

    let patch_funcs: Vec<(&str, fn(&Path, &Path, &Path) -> Result<FileInfo>)> = vec![
        ("bsdiff", utils::bsdiff::create_patch),
        ("bidiff", utils::bidiff::create_patch),
        ("zstd", utils::zstd::create_patch),
        ("lzma", utils::lzma::create_patch),
    ];

    for (name, func) in patch_funcs.iter() {
        let patch_file = args.new_file.with_extension(name);
        println!("Testing {}...", name);
        let start = Instant::now();
        func(&args.old_file, &args.new_file, &patch_file)?;
        let duration = start.elapsed();
        let patch_size = patch_file.metadata()?.len();
        println!("Time taken: {:?}, patch size: {} bytes", duration, patch_size);
    }

    Ok(())
}
