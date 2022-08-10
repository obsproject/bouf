#![allow(dead_code)]

use std::path::Path;

mod config;
mod utils;
use crate::config::Config;

fn main() {
    let conf_path = Path::new("config.example.toml");
    let conf = Config::from_file(conf_path);

    println!("Output dir: {}", &conf.env.output_dir.to_str().unwrap());
}
