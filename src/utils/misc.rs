use std::path::{Component, Path, PathBuf};

use crate::config::ObsVersion;

pub fn parse_version(version_string: &String) -> (u8, u8, u8) {
    let parts: Vec<&str> = version_string.split(".").collect();

    let major: u8 = parts[0].parse().unwrap();
    let minor: u8 = parts[1].parse().unwrap();
    let patch: u8 = parts[2].parse().unwrap();

    (major, minor, patch)
}

pub fn get_filename_version(version: &ObsVersion, short: bool) -> String {
    let mut ver = format!("{}.{}", version.version_major, version.version_minor);
    if !short || version.version_patch > 0 {
        ver += version.version_major.to_string().as_str();
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
