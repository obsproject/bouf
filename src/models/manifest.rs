use serde::{Deserialize, Serialize};

use crate::models::config::ObsVersion;

#[derive(Serialize, Deserialize, Default)]
pub struct Manifest {
    pub notes: String,
    pub packages: Vec<Package>,
    pub version_major: u8,
    pub version_minor: u8,
    pub version_patch: u8,
    pub beta: u8,
    pub rc: u8,
    pub commit: String,
    pub vc2019_redist_x64: String,
    pub vc2019_redist_x86: String,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Package {
    pub name: String,
    pub removed_files: Vec<String>,
    pub files: Vec<FileEntry>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct FileEntry {
    pub hash: String,
    pub name: String,
    pub size: u64,
}

impl Manifest {
    pub fn new() -> Self {
        Self { ..Default::default() }
    }

    pub fn with_version(mut self, ver: &ObsVersion) -> Self {
        self.version_major = ver.version_major;
        self.version_minor = ver.version_minor;
        self.version_patch = ver.version_patch;
        self.rc = ver.rc;
        self.beta = ver.beta;
        self.commit = ver.commit.to_owned();

        self
    }

    pub fn to_json(&self, pretty: bool) -> Result<String, Box<dyn std::error::Error>> {
        let res: String;
        if pretty {
            res = serde_json::to_string_pretty(&self)?;
        } else {
            res = serde_json::to_string(&self)?;
        }

        Ok(res)
    }
}
