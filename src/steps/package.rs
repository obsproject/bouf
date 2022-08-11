use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::result::Result;

#[cfg(target_os = "windows")]
use tugger_windows_codesign::{CodeSigningCertificate, SigntoolSign, SystemStore, TimestampServer};

use crate::config::Config;
use crate::utils::errors::SomeError;
use crate::utils::misc;

#[cfg(target_os = "windows")]
pub fn run_nsis(conf: &Config) -> Result<(), Box<dyn std::error::Error>> {
    // ToDo make installer name more configurable
    let new_version = &conf.obs_version.version_str;
    let tag_version = misc::get_filename_version(&conf.obs_version, false);
    let short_version = misc::get_filename_version(&conf.obs_version, true);
    // The build dir is the "install" subfolder in the output dir
    let build_dir = conf.env.output_dir.join("install").canonicalize()?;
    let build_dir_str = build_dir.into_os_string().into_string().unwrap();
    let nsis_script = conf.package.installer.nsis_script.canonicalize()?;
    let script_dir = nsis_script.parent().unwrap();

    let args: Vec<OsString> = vec![
        "/NOCD".into(),
        format!("/DTAGVERSION={}", tag_version).into(),
        format!("/DAPPVERSION={}", new_version).into(),
        format!("/DSHORTVERSION={}", short_version).into(),
        format!("/DBUILDDIR={}", build_dir_str).into(),
        "/DINSTALL64".into(),
        "/DFULL".into(),
        nsis_script.to_owned().into_os_string(),
    ];

    let output = Command::new(&conf.env.makensis_path)
        .current_dir(script_dir)
        .args(args)
        .output()?;

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
    let cert = CodeSigningCertificate::SubjectName(SystemStore::My, config.prepare.codesign.sign_name.to_owned());
    let mut sign = SigntoolSign::new(cert);
    sign.verbose()
        .file_digest_algorithm(config.prepare.codesign.sign_digest.to_owned())
        .timestamp_server(TimestampServer::Simple(config.prepare.codesign.sign_ts_serv.to_owned()))
        .sign_file(path);
    sign.run()?;

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

    let obs_path = conf.env.output_dir.join("install").canonicalize()?;
    let pdb_path = conf.env.output_dir.join("pdbs").canonicalize()?;
    let obs_zip_path = conf.env.output_dir.join(zip_name).canonicalize()?;
    let pdb_zip_path = conf.env.output_dir.join(pdb_zip_name).canonicalize()?;

    run_sevenzip(&conf.env.sevenzip_path, &obs_path, &obs_zip_path)?;
    if !conf.package.zip.skip_for_prerelease {
        run_sevenzip(&conf.env.sevenzip_path, &pdb_path, &pdb_zip_path)?;
    }

    Ok(())
}
