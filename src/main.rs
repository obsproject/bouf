use anyhow::{Context, Result};
use clap::Parser;

mod models;
mod steps;
mod utils;

use models::args::MainArgs;
use models::config::Config;
use steps::generate::Generator;
use steps::package::Packaging;
use steps::prepare::Preparator;
use utils::sign::Signer;

fn main() -> Result<()> {
    let args: MainArgs = MainArgs::parse();
    let mut conf = Config::from_file(args.config.as_path())?;

    println!("[+] Verifying config validity...");
    conf.apply_args(&args).context("[!] Config invalid")?;
    println!("[+] Config Ok!");

    println!("bouf process started with the following locations set:");
    println!(" - Input dir: {}", &conf.env.input_dir.display());
    println!(" - Previous versions dir: {}", &conf.env.previous_dir.display());
    println!(" - Output dir: {}", &conf.env.output_dir.display());

    if !args.skip_preparation {
        let prep = Preparator::init(&conf);
        prep.run().context("[!] Preparation failed")?;
    } else {
        println!("[*] Skipped preparation, this will also disable installer/zip creation.")
    }

    // Create deltas and manifest
    println!("[+] Creating manifest and patches...");
    let generator = Generator::init(&conf, !args.skip_preparation);
    let mut manifest = generator
        .run(args.skip_patches)
        .context("[!] Error during generator run")?;

    let packager = Packaging::init(&conf);
    // Create NSIS/ZIP
    if !args.skip_installer && !args.skip_preparation {
        println!("[+] Creating Installer");
        packager.run_nsis().context("[!] NSIS creation/signing failed")?;
    } else {
        println!("[*] Skipping installer creation...")
    }

    if !args.skip_preparation {
        // Create PDB and install folder ZIPs
        println!("[+] Creating zip files...");
        packager.create_zips().context("[!] Creating zip files failed")?;
        println!("[+] ZIP files created successfully!")
    } else {
        println!("[*] Skipping ZIP creation as preparation was skipped...")
    }

    // Sign manifest if it was created
    println!("[+] Finalising manifest...");
    let mf = packager
        .finalise_manifest(&mut manifest)
        .context("[!] Finalising manifest failed")?;

    if !conf.package.updater.skip_sign {
        println!("[+] Signing manifest...");
        let mut signer = Signer::init(conf.package.updater.private_key.as_ref());
        signer.sign_file(&mf).context("[!] Signing file failed")?;
    }

    if !args.skip_preparation && conf.post.copy_to_old {
        println!("[+] Copying install dir to previous version directory...");
        steps::post::copy_to_old(&conf).context("[!] Copying files failed")?;
    }

    println!("*** Finished! ***");
    Ok(())
}
