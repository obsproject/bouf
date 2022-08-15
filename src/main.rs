use std::process::exit;

mod steps;
mod utils;

use clap::Parser;

use crate::utils::args::MainArgs;
use crate::utils::config::Config;

fn main() {
    let args: MainArgs = MainArgs::parse();
    let mut conf = Config::from_file(args.config.as_path());
    conf.apply_args(&args);

    println!("[+] Verifying config validity...");
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
    println!("[+] Creating manifest and patches...");
    let mut manifest = steps::generate::create_manifest_and_patches(&conf, args.skip_patches, false);

    // Create NSIS/ZIP
    if !args.skip_installer {
        println!("[+] Creating Installer");
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
    println!("[+] Finalising manifest...");
    let mf = steps::package::finalise_manifest(&conf, &mut manifest);
    if let Err(e) = mf {
        println!("[!] Finalising manifest failed: {}", e);
        exit(1)
    }

    if !conf.package.updater.skip_sign {
        println!("[+] Signing manifest...");
        // ToDo fix this mess
        /* let mut privkey = args.private_key;
        if conf.package.updater.private_key.exists() && conf.package.updater.private_key.ends_with(".pem") {
            privkey = Some(conf.package.updater.private_key);
        } */

        let key = utils::sign::load_key(Some(conf.package.updater.private_key));
        if let Err(e) = key {
            println!("[!] Loading singing key failed: {}", e);
            exit(1)
        }

        let res = utils::sign::sign_file(&key.unwrap(), &mf.unwrap());
        if let Err(e) = res {
            println!("[!] Signing file failed: {}", e);
            exit(1)
        }
    }

    println!("*** Finished! ***");
}
