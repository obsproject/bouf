use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Result};

use crate::models::config::ObsVersion;

/// Parses a version string such as "28.0.0-rc1" to version struct
pub fn parse_version(version_string: &String) -> ObsVersion {
    let parts: Vec<&str> = version_string.split("-").collect();
    let numbers: Vec<&str> = parts[0].split(".").collect();

    let mut version = ObsVersion { ..Default::default() };

    version.version_str = parts[0].to_string();
    version.version_major = numbers[0].parse().unwrap();
    version.version_minor = numbers[1].parse().unwrap();
    version.version_patch = numbers[2].parse().unwrap();

    if parts.len() > 1 {
        let suffix = parts[1];
        // Parse -beta<Num> and -rc<Num> suffixes
        if suffix.starts_with("beta") {
            version.beta = suffix[4..].parse().unwrap();
        } else if suffix.starts_with("rc") {
            version.rc = suffix[2..].parse().unwrap();
        } else {
            panic!("Invalid version string!")
        }
    }

    version
}

/// Get the version string used in filenames, optionally as a short version
/// (dropping the patch part if it is "0")
pub fn get_filename_version(version: &ObsVersion, short: bool) -> String {
    let mut ver = format!("{}.{}", version.version_major, version.version_minor);
    if !short || version.version_patch > 0 {
        ver += format!(".{}", version.version_patch).as_str();
    }

    if version.beta > 0 {
        ver += format!("-beta{}", version.beta).as_str();
    } else if version.rc > 0 {
        ver += format!("-rc{}", version.rc).as_str();
    } else if !version.commit.is_empty() {
        ver += format!("-g{}", &version.commit[..8]).as_str();
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

/// Attempt to create a canonical path from a relative one where some elements may not exist (yet).
/// This works by searching for the longest chain of components of the specified path that does
/// exist, then appending the remaining ones.
/// For instance, the output folder may not exist, but we may still want an absolute path to it.
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

fn check_for_command(name: &str) -> Result<()> {
    let mut child = Command::new(name);

    match child.spawn() {
        Ok(mut s) => s.kill().expect("Could not kill spawned process"),
        Err(e) => bail!("Failed to find \"{}\" command: {} ({})", name, e, e.kind()),
    };

    Ok(())
}

/// Checks if a binary path is valid, alternatively falls back to
/// checking if the specified string exists as a binary in $PATH/%PATH%
/// (This is probably bad for many cases, but here it makes sense I swear)
pub fn check_binary_path(path: &mut PathBuf) -> Result<()> {
    if fs::metadata(&path).is_ok() {
        return Ok(());
    }
    let fname = path.file_name().unwrap().to_str().unwrap();
    check_for_command(fname)?;
    *path = fname.into();

    Ok(())
}

#[cfg(test)]
mod misc_tests {
    use super::*;
    use crate::models::config::ObsVersion;

    #[test]
    fn test_parse_version() {
        // Release version
        let str_ver = "28.0.0".to_string();
        let ref_ver = ObsVersion {
            version_str: "28.0.0".into(),
            version_major: 28,
            ..Default::default()
        };
        let ver = parse_version(&str_ver);
        assert_eq!(ver, ref_ver);
        // Beta version
        let str_ver = "28.1.0-beta2".to_string();
        let ref_ver = ObsVersion {
            version_str: "28.1.0".into(), // This string does not include suffixes
            version_major: 28,
            version_minor: 1,
            beta: 2,
            ..Default::default()
        };
        let ver = parse_version(&str_ver);
        assert_eq!(ver, ref_ver);
    }

    #[test]
    fn test_file_version() {
        let mut version = ObsVersion {
            version_major: 28,
            version_minor: 1,
            version_patch: 1,
            beta: 1,
            ..Default::default()
        };

        // Test long beta version
        let ver_long = get_filename_version(&version, false);
        assert_eq!(ver_long, "28.1.1-beta1");

        // Test short release version
        version.beta = 0;
        version.version_patch = 0;
        let ver_short = get_filename_version(&version, true);
        assert_eq!(ver_short, "28.1");

        // Test nightly version
        version.commit = "abcdef12".to_string();
        let ver_short = get_filename_version(&version, false);
        assert_eq!(ver_short, "28.1.0-gabcdef12");
    }
}
