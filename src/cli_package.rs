#![allow(dead_code)]

use std::path::Path;

mod utils;

fn main() {
    let conf_path = Path::new("config.example.toml");
    let conf = utils::config::Config::from_file(conf_path);

    println!("Output dir: {}", &conf.env.output_dir.to_str().unwrap());
}
