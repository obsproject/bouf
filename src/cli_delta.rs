#![allow(dead_code)]

use std::fs;
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
    #[clap(long, value_parser, value_name = "new build")]
    new: Option<PathBuf>,
    #[clap(long, value_parser, value_name = "old builds")]
    old: Option<PathBuf>,
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
    conf.set_dirs(args.new, args.out, args.old);
    // Override branch if desired
    if let Some(branch) = args.branch {
        conf.env.branch = branch;
    }

    let manifest = steps::generate::create_patches(&conf);

    let manifest_filename = format!("manifest_{}.json", conf.env.branch);
    let manifest_file = conf.env.output_dir.join(manifest_filename);
    serde_json::to_string_pretty(&manifest).ok().and_then(|j| {
        File::create(manifest_file.as_path())
            .ok()
            .and_then(|mut f: File| f.write_all(&j.as_bytes()).ok())
    });
}
