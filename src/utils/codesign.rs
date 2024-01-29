#![allow(unused_variables)]

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use log::{debug, error, info, warn};
#[cfg(windows)]
use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY};
#[cfg(windows)]
use winreg::RegKey;

use crate::models::config::CodesignOptions;

const MAX_FILES: usize = 1;
// std::process::Output's status is returned as an i32, but on Windows it's a u32
const IGNORE_STATUS: i32 = 0xc0000374u32 as i32;

#[cfg(windows)]
pub fn sign(files: Vec<PathBuf>, opts: &CodesignOptions) -> Result<()> {
    let signtool = locate_signtool()?;
    debug!("Signtool found at {:?}", signtool);

    let mut args: Vec<OsString> = vec![
        "sign".into(),
        "/fd".into(),
        opts.sign_digest.to_owned().into(),
        "/t".into(),
        opts.sign_ts_serv.to_owned().into(),
    ];

    if let Some(name) = &opts.sign_name {
        args.push("/n".into());
        args.push(name.to_owned().into());
    } else if let Some(file) = &opts.sign_cert_file {
        args.push("/f".into());
        args.push(file.into());
    }

    if let Some(kms_id) = &opts.sign_kms_key_id {
        args.push("/csp".into());
        args.push("Google Cloud KMS Provider".into());
        args.push("/kc".into());
        args.push(kms_id.into());
    }

    let slices = (files.len() + MAX_FILES - 1) / MAX_FILES;
    let mut ctr = 0;

    for chunk in files.chunks(MAX_FILES) {
        ctr += 1;
        let mut chunk_args = args.to_owned();
        for x in chunk {
            chunk_args.push(x.to_owned().into_os_string())
        }

        info!(" => Running signtool ({ctr}/{slices})...");
        if chunk.len() == 1 {
            debug!("  + Signing file {:?}", chunk[0]);
        }
        let output = Command::new(&signtool).args(chunk_args).output()?;

        if !output.status.success() {
            // Annoying error code that seems to only happen *after* successfully signing...
            if output.status.code().unwrap() == IGNORE_STATUS {
                warn!("signtool returned ignored non-success status: {}", output.status);
                continue;
            }
            error!("signtool returned non-success status: {}", output.status);
            std::io::stdout().write_all(&output.stdout)?;
            std::io::stderr().write_all(&output.stderr)?;

            return Err(anyhow!("signtool failed (see stdout/stderr for details)"));
        }
    }

    Ok(())
}

#[cfg(unix)]
pub fn sign(files: Vec<PathBuf>, opts: &CodesignOptions) -> Result<()> {
    use log::warn;
    warn!("Codesigning is not (yet) supported on this platform.");

    Ok(())
}

// Based on https://github.com/forbjok/rust-codesign/blob/master/lib/src/signtool.rs (Apache-2/MIT)
// But simplified to be 64-bit only, and slightly shittier error handling

#[cfg(windows)]
fn locate_signtool() -> Result<PathBuf> {
    const INSTALLED_ROOTS_REGKEY_PATH: &str = r"SOFTWARE\Microsoft\Windows Kits\Installed Roots";
    const KITS_ROOT_REGVALUE_NAME: &str = r"KitsRoot10";

    let installed_roots_key_path = Path::new(INSTALLED_ROOTS_REGKEY_PATH);

    // Open 32-bit HKLM "Installed Roots" key
    let installed_roots_key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(installed_roots_key_path, KEY_READ | KEY_WOW64_32KEY)
        .with_context(|| format!("Error opening registry key: {INSTALLED_ROOTS_REGKEY_PATH}"))?;

    // Get the Windows SDK root path
    let kits_root_10_path: String = installed_roots_key
        .get_value(KITS_ROOT_REGVALUE_NAME)
        .with_context(|| format!("Error getting {KITS_ROOT_REGVALUE_NAME} value from registry!"))?;

    // Construct Windows SDK bin path
    let kits_root_10_bin_path = Path::new(&kits_root_10_path).join("bin");

    let mut installed_kits: Vec<String> = installed_roots_key
        .enum_keys()
        /* Report and ignore errors, pass on values. */
        .filter_map(|res| match res {
            Ok(v) => Some(v),
            Err(err) => {
                error!("Error enumerating installed root keys: {err}");
                None
            }
        })
        .collect();

    // Sort installed kits
    installed_kits.sort();
    let kit_bin_paths: Vec<PathBuf> = installed_kits
        .iter()
        .rev()
        .map(|kit| kits_root_10_bin_path.join(kit))
        .collect();

    let mut found_path: Option<PathBuf> = None;

    for kit_bin_path in kit_bin_paths.iter() {
        let signtool_path = kit_bin_path.join("x64").join("signtool.exe");
        if signtool_path.exists() {
            debug!("signtool version found at {:?}", signtool_path);
            if found_path.is_none() {
                found_path = Some(signtool_path);
            }
        }
    }

    match found_path {
        Some(path) => Ok(path),
        None => Err(anyhow!("Signtool was not found!")),
    }
}
