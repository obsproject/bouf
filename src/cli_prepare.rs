#![allow(dead_code)]

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Parser;

mod steps;
mod utils;

// Dedicated Delta Patch builder for testing or regenerating patches if required.

#[derive(Parser, Debug)]
#[clap(about, long_about = None)]
struct Args {
    // Required
    #[clap(short, long, value_parser, value_name = "Config file")]
    config: PathBuf,
    #[clap(long, value_parser, value_name = "OBS main version (Major.Minor.Patch)")]
    version: String,

    // Optional version suffix
    #[clap(long, value_parser, value_name = "Beta number")]
    beta: Option<u8>,
    #[clap(long, value_parser, value_name = "RC number")]
    rc: Option<u8>,
    #[clap(long, value_parser, value_name = "Beta branch")]
    branch: Option<String>,

    // Optional overrides
    #[clap(short, long, value_parser, default_value_t = false)]
    delete_old: bool,
    #[clap(long, value_parser, value_name = "new build")]
    new: Option<PathBuf>,
    #[clap(long, value_parser, value_name = "output dir")]
    out: Option<PathBuf>,
}

fn main() {
    let args: Args = Args::parse();

    let mut conf = utils::config::Config::from_file(args.config.as_path());
    conf.set_version(
        &args.version,
        args.beta.unwrap_or_default(),
        args.rc.unwrap_or_default(),
    );
    conf.set_dirs(args.new, args.out, None);
    // Override branch if desired
    if let Some(branch) = args.branch {
        conf.env.branch = branch;
    }

    println!("Started prepare step with following locations:");
    println!(" - Input dir: {}", &conf.env.input_dir.to_str().unwrap());
    println!(" - Output dir: {}", &conf.env.output_dir.to_str().unwrap());

    steps::prepare::ensure_output_dir(&conf.env.output_dir, args.delete_old)
        .expect("Failed ensuring output dir exists/is empty.");
    // Copy build to "install"  dir
    steps::prepare::copy(&conf.env.input_dir, &conf.env.output_dir, &conf.prepare.copy)
        .expect("Failed copying new build!");
    // Codesign files
    steps::prepare::codesign(&conf.env.output_dir, &conf.prepare.codesign);
    // Move/Strip PDBs
    steps::prepare::strip_pdbs(&conf.env.output_dir, &conf.prepare.strip_pdbs, &conf.env);
}
