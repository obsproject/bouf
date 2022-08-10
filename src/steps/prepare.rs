use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::result::Result;

#[cfg(target_os = "windows")]
use tugger_windows_codesign::{CodeSigningCertificate, SigntoolSign, SystemStore, TimestampServer};

use hashbrown::HashSet;
use walkdir::{DirEntry, WalkDir};

use crate::config::{CodesignOptions, CopyOptions, StripPDBOptions};
use crate::utils::errors;
use crate::utils::misc;

pub fn ensure_output_dir(out_path: &PathBuf, delete_old: bool) -> Result<(), Box<dyn std::error::Error>> {
    let out_path = misc::normalize_path(&out_path);

    if !out_path.read_dir()?.next().is_none() {
        if !delete_old {
            return Err(Box::new(errors::SomeError("Folder not empty".into())));
        }
        println!("Deleting previous output dir...");
        std::fs::remove_dir_all(&out_path);
    }

    std::fs::create_dir_all(&out_path)?;
    Ok(())
}

pub fn copy(in_path: &PathBuf, out_path: &PathBuf, opts: &CopyOptions) -> Result<(), Box<dyn std::error::Error>> {
    let out_path = misc::normalize_path(&out_path.join("install"));
    let inp_path = misc::normalize_path(&in_path);
    let mut overrides: HashSet<&String> = HashSet::new();
    // Convert to hash set for fast lookup
    opts.overrides.iter().for_each(|(old, _)| {
        overrides.insert(old);
    });

    println!(
        "[+] Copying build from \"{}\" to \"{}\"...",
        inp_path.display(),
        out_path.display()
    );
    std::fs::create_dir_all(&out_path)?;

    // Walk dir, honor overrides where necessary
    for file in WalkDir::new(&inp_path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| !e.file_type().is_dir())
    {
        let file: DirEntry = file;
        // Get a path relative to the input directory for lookup/copy path
        let relative_path = file.path().strip_prefix(&inp_path).unwrap().to_str().unwrap();
        let relative_path_str = String::from(relative_path).replace("\\", "/");
        // Check against overrides
        if overrides.contains(&relative_path_str) {
            continue;
        }
        // Check relative path against excludes
        if opts.excludes.iter().find(|&x| relative_path_str.contains(x)).is_some() {
            continue;
        }
        let file_path = out_path.join(relative_path);
        // Ensure dir structure exists
        if let Some(_parent) = file_path.parent() {
            fs::create_dir_all(_parent)?;
        }
        fs::copy(file.path(), file_path)?;
    }

    // Copy override files over
    opts.overrides.iter().for_each(|(ins_path, src_path)| {
        if !fs::metadata(src_path).is_ok() {
            panic!("Override file \"{}\" does not exist!", src_path)
        }

        let full_path = out_path.join(ins_path);
        if let Some(_parent) = full_path.parent() {
            fs::create_dir_all(_parent);
        }
        fs::copy(src_path, full_path);
    });

    Ok(())
}

// Move PDBs (except excluded) to separate dir, then strip remaining ones
pub fn strip_pdbs(path: &PathBuf, opts: &StripPDBOptions) -> Result<(), Box<dyn std::error::Error>> {
    let inp_path = misc::normalize_path(&path.join("install"));
    let out_path = misc::normalize_path(&path.join("pdbs"));

    println!(
        "[+] Copying/stripping PDBs from \"{}\" to \"{}\"...",
        inp_path.display(),
        out_path.display()
    );

    for file in WalkDir::new(&inp_path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| !e.file_type().is_dir())
    {
        let file: DirEntry = file;
        let relative_path = file.path().strip_prefix(&inp_path).unwrap().to_str().unwrap();
        if !relative_path.ends_with(".pdb") {
            continue;
        }
        let relative_path_str = String::from(relative_path).replace("\\", "/");
        let new_path = out_path.join(relative_path);
        if let Some(_parent) = new_path.parent() {
            fs::create_dir_all(_parent)?;
        }

        if opts.exclude.iter().find(|&x| relative_path_str.contains(x)).is_some() {
            fs::copy(file.path(), &new_path);
            continue;
        }
        fs::rename(file.path(), &new_path)?;

        // Finally, run PDBCopy
        Command::new(&opts.pdbcopy_path)
            .args([new_path.as_os_str(), file.path().as_os_str(), OsStr::new("-p")])
            .output()
            .expect("failed to run pdbcopy");
    }
    Ok(())
}

// Sign all eligible files in a folder using Signtool
#[cfg(target_os = "windows")]
pub fn codesign(path: &PathBuf, opts: &CodesignOptions) -> Result<(), Box<dyn std::error::Error>> {
    if opts.skip_sign {
        return Ok(());
    }
    let inp_path = misc::normalize_path(&path.join("install"));

    println!("[+] Signing files in \"{}\"", inp_path.display());
    let cert = CodeSigningCertificate::SubjectName(SystemStore::My, opts.sign_name.to_owned());
    let mut sign = SigntoolSign::new(cert);
    sign.verbose()
        .file_digest_algorithm(opts.sign_digest.to_owned())
        .timestamp_server(TimestampServer::Simple(opts.sign_ts_serv.to_owned()));

    for file in WalkDir::new(&inp_path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| !e.file_type().is_dir())
    {
        let file: DirEntry = file;
        let relative_path = file.path().to_str().unwrap();
        if !opts.sign_exts.iter().find(|&x| relative_path.ends_with(x)).is_some() {
            continue;
        }
        sign.sign_file(file.path().canonicalize()?);
    }
    println!("[+] Running signtool...");
    sign.run();

    Ok(())
}

#[cfg(target_os = "linux")]
pub fn codesign(in_path: &PathBuf, opts: &CodesignOptions) -> Result<(), Box<dyn std::error::Error>> {
    println!("Codesigning is not (yet) supported on this platform.");

    Ok()
}
