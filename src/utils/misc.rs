use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use crate::utils::config::ObsVersion;
use crate::utils::errors::SomeError;

pub fn parse_version(version_string: &String) -> (u8, u8, u8, u8, u8) {
    let parts: Vec<&str> = version_string.split("-").collect();
    let mut beta: u8 = 0;
    let mut rc: u8 = 0;

    if parts.len() > 1 {
        let suffix = parts[1];

        // Parse -beta<Num> and -rc<Num> suffixes
        if suffix.starts_with("beta") {
            beta = suffix[4..].parse().unwrap();
        } else if suffix.starts_with("rc") {
            rc = suffix[2..].parse().unwrap();
        } else {
            panic!("Invalid version string!")
        }
    }

    let numbers: Vec<&str> = parts[0].split(".").collect();
    let major: u8 = numbers[0].parse().unwrap();
    let minor: u8 = numbers[1].parse().unwrap();
    let patch: u8 = numbers[2].parse().unwrap();

    (major, minor, patch, beta, rc)
}

pub fn get_filename_version(version: &ObsVersion, short: bool) -> String {
    let mut ver = format!("{}.{}", version.version_major, version.version_minor);
    if !short || version.version_patch > 0 {
        ver += format!(".{}", version.version_major).as_str();
    }

    if version.beta > 0 {
        ver += format!("-beta{}", version.beta).as_str();
    } else if version.rc > 0 {
        ver += format!("-rc{}", version.rc).as_str();
    }

    ver
}

// Nicked from Cargo
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut components = path.components().peekable();
    let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek().cloned() {
        components.next();
        PathBuf::from(c.as_os_str())
    } else {
        PathBuf::new()
    };

    for component in components {
        match component {
            Component::Prefix(..) => unreachable!(),
            Component::RootDir => {
                ret.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                ret.pop();
            }
            Component::Normal(c) => {
                ret.push(c);
            }
        }
    }
    ret
}

/// Attempt to create canonical path
pub fn recursive_canonicalize(path: &PathBuf) -> PathBuf {
    let mut out_path = PathBuf::new();

    for component in path.components() {
        let tmp = out_path.join(component);
        // As long as component is canonizable, just replace it, otherwise just push the components
        if let Ok(_canon) = tmp.canonicalize() {
            out_path = _canon;
        } else {
            out_path.push(component);
        }
    }

    out_path
}

fn check_for_command(name: &str) -> Result<(), SomeError> {
    let mut child = Command::new(name);

    match child.spawn() {
        Ok(mut s) => s.kill().expect("Could not kill spawned process"),
        Err(e) => {
            let msg = format!("Failed to find \"{}\" command: {} ({})", name, e, e.kind());
            return Err(SomeError(msg));
        }
    };

    Ok(())
}

pub fn check_binary_path(path: &mut PathBuf) -> Result<(), SomeError> {
    if fs::metadata(&path).is_ok() {
        return Ok(());
    }
    let fname = path.file_name().unwrap().to_str().unwrap();
    check_for_command(fname)?;
    *path = fname.into();

    Ok(())
}
