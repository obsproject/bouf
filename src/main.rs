use std::process::exit;

mod models;
mod steps;
mod utils;

use clap::Parser;

use crate::steps::package::Packaging;
use crate::utils::sign::Signer;
use models::args::MainArgs;
use models::config::Config;
use models::manifest::Manifest;
use steps::generate::Generator;
use steps::prepare::Preparator;

fn main() {
    let args: MainArgs = MainArgs::parse();
    let mut conf = Config::from_file(args.config.as_path());

    println!("[+] Verifying config validity...");
    if let Err(err) = conf.apply_args(&args) {
        println!("[!] Config invalid: {}", err);
        exit(1)
    } else {
        println!("[+] Config Ok!")
    }

    println!("bouf process started with the following locations set:");
    println!(" - Input dir: {}", &conf.env.input_dir.display());
    println!(" - Previous versions dir: {}", &conf.env.previous_dir.display());
    println!(" - Output dir: {}", &conf.env.output_dir.display());

    if !args.skip_preparation {
        let prep = Preparator::init(&conf);
        if let Err(err) = prep.run() {
            println!("[!] Preparation failed: {}", err);
            exit(1)
        }
    } else {
        println!("[*] Skipped preparation, this will also disable installer/zip creation.")
    }

    // Create deltas and manifest
    println!("[+] Creating manifest and patches...");
    let mut manifest: Manifest;
    let generator = Generator::init(&conf, !args.skip_preparation);

    match generator.run(args.skip_patches) {
        Err(err) => {
            println!("[!] Error during generator run: {}", err);
            exit(1)
        }
        Ok(_manifest) => manifest = _manifest,
    }

    let packager = Packaging::init(&conf);
    // Create NSIS/ZIP
    if !args.skip_installer && !args.skip_preparation {
        println!("[+] Creating Installer");
        if let Err(e) = packager.run_nsis() {
            println!("[!] NSIS creation/signing failed: {}", e);
            exit(1)
        }
    } else {
        println!("[*] Skipping installer creation...")
    }

    if !args.skip_preparation {
        // Create PDB and install folder ZIPs
        println!("[+] Creating zip files...");
        if let Err(err) = packager.create_zips() {
            println!("[!] Creating zip files failed: {}", err);
            exit(1)
        }
        println!("[+] ZIP files created successfully!")
    } else {
        println!("[*] Skipping ZIP creation as preparation was skipped...")
    }

    // Sign manifest if it was created
    println!("[+] Finalising manifest...");
    let mf = packager.finalise_manifest(&mut manifest);
    if let Err(e) = mf {
        println!("[!] Finalising manifest failed: {}", e);
        exit(1)
    }

    if !conf.package.updater.skip_sign {
        println!("[+] Signing manifest...");
        let mut signer = Signer::init(conf.package.updater.private_key.as_ref());
        if let Err(e) = signer.sign_file(&mf.unwrap()) {
            println!("[!] Signing file failed: {}", e);
            exit(1)
        }
    }

    if !args.skip_preparation && conf.post.copy_to_old {
        println!("[+] Copying install dir to previous version directory...");
        let res = steps::post::copy_to_old(&conf);
        if let Err(e) = res {
            println!("[!] Copying files failed: {}", e);
            exit(1)
        }
    }

    println!("*** Finished! ***");
}
