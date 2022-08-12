use std::ffi::OsString;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::result::Result;

use serde_json;

use crate::steps::generate::Manifest;
use crate::utils::codesign::sign;
use crate::utils::config::Config;
use crate::utils::errors::SomeError;
use crate::utils::hash::hash_file;
use crate::utils::misc;

#[cfg(target_os = "windows")]
pub fn run_nsis(conf: &Config) -> Result<(), Box<dyn std::error::Error>> {
    // ToDo make installer name more configurable
    let new_version = &conf.obs_version.version_str;
    let tag_version = misc::get_filename_version(&conf.obs_version, false);
    let short_version = misc::get_filename_version(&conf.obs_version, true);
    let nsis_script = conf.package.installer.nsis_script.canonicalize()?;

    // The build dir is the "install" subfolder in the output dir
    let build_dir = conf.env.output_dir.join("install").canonicalize()?;
    let mut build_dir_str = build_dir.into_os_string().into_string().unwrap();
    // Sanitise build dir string for NSIS
    if build_dir_str.starts_with("\\") {
        build_dir_str = build_dir_str.strip_prefix("\\\\?\\").unwrap().to_string();
    }

    let args: Vec<OsString> = vec![
        format!("/DTAGVERSION={}", tag_version).into(),
        format!("/DAPPVERSION={}", new_version).into(),
        format!("/DSHORTVERSION={}", short_version).into(),
        format!("/DBUILDDIR={}", build_dir_str).into(),
        "/DINSTALL64".into(),
        "/DFULL".into(),
        nsis_script.to_owned().into_os_string(),
    ];

    println!(" => Running NSIS...");
    let output = Command::new(&conf.env.makensis_path).args(args).output()?;

    if !output.status.success() {
        println!("MakeNSIS returned non-success status: {}", output.status);
        std::io::stdout().write_all(&output.stdout)?;
        std::io::stderr().write_all(&output.stderr)?;

        Err(Box::new(SomeError(
            "MakeNSIS failed (see stdout/stderr for details)".to_string(),
        )))
    } else {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
pub fn run_nsis(conf: &Config) -> Result<(), Box<dyn std::error::Error>> {
    println!("Creating an installer is not (yet) supported on this platform.");

    Ok(())
}

#[cfg(target_os = "windows")]
pub fn sign_installer(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let filename = format!(
        "OBS-Studio-{}-Full-Installer-x64.exe",
        misc::get_filename_version(&config.obs_version, true)
    );
    let path = config.env.output_dir.join(filename).canonicalize()?;

    println!("[+] Signing installer file \"{}\"", path.display());
    let files: Vec<PathBuf> = vec![path];
    sign(files, &config.prepare.codesign)?;

    Ok(())
}

#[cfg(target_os = "linux")]
pub fn sign_installer(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    println!("Singing an installer is not (yet) supported on this platform.");

    Ok(())
}

fn run_sevenzip(sevenzip: &PathBuf, in_path: &PathBuf, out_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<OsString> = vec![
        "a".into(),
        "-r".into(),
        "-y".into(),
        "--".into(),
        out_path.to_owned().into_os_string(),
        in_path.to_owned().into_os_string(),
    ];

    let output = Command::new(sevenzip).args(args).output()?;

    if !output.status.success() {
        println!("7-zip returned non-success status: {}", output.status);
        std::io::stdout().write_all(&output.stdout)?;
        std::io::stderr().write_all(&output.stderr)?;

        Err(Box::new(SomeError(
            "7-zip failed (see stdout/stderr for details)".to_string(),
        )))
    } else {
        Ok(())
    }
}

pub fn create_zips(conf: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let short_version = misc::get_filename_version(&conf.obs_version, true);
    let zip_name = conf.package.zip.name.replace("{version}", &short_version);
    let pdb_zip_name = conf.package.zip.pdb_name.replace("{version}", &short_version);

    let obs_path = conf.env.output_dir.join("install/*");
    let pdb_path = conf.env.output_dir.join("pdbs/*");
    let obs_zip_path = conf.env.output_dir.join(zip_name);
    let pdb_zip_path = conf.env.output_dir.join(pdb_zip_name);

    run_sevenzip(&conf.env.sevenzip_path, &obs_path, &obs_zip_path)?;
    if !conf.package.zip.skip_for_prerelease {
        run_sevenzip(&conf.env.sevenzip_path, &pdb_path, &pdb_zip_path)?;
    }

    Ok(())
}

fn run_pandoc(path: &PathBuf, out_path: &PathBuf) -> Result<String, Box<dyn std::error::Error>> {
    let args: Vec<OsString> = vec![
        "--from".into(),
        "markdown".into(),
        "--to".into(),
        "html".into(),
        path.to_owned().into_os_string(),
    ];

    let output = Command::new("pandoc").args(args).output()?;

    if !output.status.success() {
        println!("pandoc returned non-success status: {}", output.status);
        std::io::stdout().write_all(&output.stdout)?;
        std::io::stderr().write_all(&output.stderr)?;
        Err(Box::new(SomeError(
            "pandoc failed (see stdout/stderr for details)".to_string(),
        )))
    } else {
        Ok(String::from_utf8(output.stdout)?)
    }
}

pub fn finalise_manifest(conf: &Config, manifest: &mut Manifest) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let manifest_filename = format!("manifest_{}.json", conf.env.branch);
    let manifest_file = conf.env.output_dir.join(manifest_filename);

    // Add VC hash
    let hash = hash_file(&conf.package.updater.vc_redist_path);
    manifest.vc2019_redist_x64 = hash.hash;
    // Add notes
    manifest.notes = run_pandoc(&conf.package.updater.notes_files, &conf.env.output_dir)?;

    let json_str = serde_json::to_string_pretty(&manifest)?;
    let mut f = File::create(manifest_file.as_path())?;
    f.write_all(&json_str.as_bytes())?;

    Ok(manifest_file)
}
