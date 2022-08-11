use std::path::PathBuf;
use std::process::exit;

mod config;
mod steps;
mod utils;

use clap::Parser;

use crate::config::{Config, MainArgs};
use crate::steps::generate::Manifest;

fn main() {
    let args: MainArgs = MainArgs::parse();
    let mut conf = Config::from_file(args.config.as_path());
    conf.apply_args(&args);

    println!("[+] Verifying config valididty...");
    match conf.validate(true, true) {
        Ok(_) => println!("[+] Config Ok!"),
        Err(err) => {
            println!("[!] Config invalid: {}", err);
            exit(1)
        }
    };

    println!("bouf process started with the following locations set:");
    println!(" - Input dir: {}", &conf.env.input_dir.to_str().unwrap());
    println!(" - Previous versions dir: {}", &conf.env.previous_dir.to_str().unwrap());
    println!(" - Output dir: {}", &conf.env.output_dir.to_str().unwrap());

    steps::prepare::ensure_output_dir(&conf.env.output_dir, args.clear_output)
        .expect("Failed ensuring output dir exists/is empty.");
    // Copy build to "install"  dir
    steps::prepare::copy(&conf.env.input_dir, &conf.env.output_dir, &conf.prepare.copy)
        .expect("Failed copying new build!");
    // Codesign files
    steps::prepare::codesign(&conf.env.output_dir, &conf.prepare.codesign).expect("Failed to run codesigning");
    // Move/Strip PDBs
    steps::prepare::strip_pdbs(&conf.env.output_dir, &conf.prepare.strip_pdbs, &conf.env)
        .expect("Failed to strip PDBs");

    // Create deltas and manifest
    let mut manifest: Option<Manifest> = None;
    if !args.skip_patches {
        println!("[+] Creating delta patches...");
        manifest = Some(steps::generate::create_patches(&conf));
    } else {
        println!("[*] Skipping delta patch generation...");
    }

    // Create NSIS/ZIP
    if !args.skip_installer {
        println!("[+] Running NSIS...");
        if let Err(e) = steps::package::run_nsis(&conf) {
            println!("[!] NSIS failed: {}", e);
            exit(1)
        }
        println!("[+] NSIS completed successfully!");

        if !conf.package.installer.skip_sign {
            if let Err(e) = steps::package::sign_installer(&conf) {
                println!("[!] Signing installer failed: {}", e);
            }
            println!("[+] Installer signed successfully!");
        }
    } else {
        println!("[*] Skipping installer creation...")
    }

    // Create PDB and install folder ZIPs
    println!("[+] Creating zip files...");
    match steps::package::create_zips(&conf) {
        Ok(_) => println!("[+] ZIP files created successfully!"),
        Err(err) => {
            println!("[!] Creating zip files failed: {}", err);
            exit(1)
        }
    }

    // Sign manifest if it was created
    if let Some(mut manifest) = manifest {
        // ToDo write vc redist hash, convert notes, write and sign manifest

        if !conf.package.updater.skip_sign {}
    }
}
