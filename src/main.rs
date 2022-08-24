use std::process::exit;

mod models;
mod steps;
mod utils;

use clap::Parser;

use models::args::MainArgs;
use models::config::Config;
use models::manifest::Manifest;
use steps::generate::Generator;
use steps::prepare::Preparator;

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
    println!(" - Input dir: {}", &conf.env.input_dir.display());
    println!(" - Previous versions dir: {}", &conf.env.previous_dir.display());
    println!(" - Output dir: {}", &conf.env.output_dir.display());

    if !args.skip_preparation {
        let prep = Preparator::init(&conf);
        match prep.run() {
            Ok(_) => (),
            Err(err) => {
                println!("[!] Preparation failed: {}", err);
                exit(1)
            }
        }
    } else {
        println!("[*] Skipp preparation, this will also disable installer/zip creation.")
    }

    // Create deltas and manifest
    println!("[+] Creating manifest and patches...");
    let mut manifest: Manifest;
    let generator = Generator::init(&conf, !args.skip_preparation);

    match generator.run(args.skip_patches) {
        Ok(_manifest) => manifest = _manifest,
        Err(err) => {
            println!("[!] Error during generator run: {}", err);
            exit(1)
        }
    }

    // Create NSIS/ZIP
    if !args.skip_installer && !args.skip_preparation {
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

    if !args.skip_preparation {
        // Create PDB and install folder ZIPs
        println!("[+] Creating zip files...");
        match steps::package::create_zips(&conf) {
            Ok(_) => println!("[+] ZIP files created successfully!"),
            Err(err) => {
                println!("[!] Creating zip files failed: {}", err);
                exit(1)
            }
        }
    } else {
        println!("[*] Skipping ZIP creation as preparation was skipped...")
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
        let key = utils::sign::load_key(&conf.package.updater.private_key);
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

    if !args.skip_preparation {
        if conf.post.copy_to_old {
            println!("[+] Copying install dir to previous version directory...");
            let res = steps::post::copy_to_old(&conf);
            if let Err(e) = res {
                println!("[!] Copying files failed: {}", e);
                exit(1)
            }
        }
    }

    println!("*** Finished! ***");
}
