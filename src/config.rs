use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use toml;

use crate::utils::misc::parse_version;

fn get_default_branch() -> String {
    String::from("stable")
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub env: EnvOptions,
    pub prepare: PreparationOptions,
    pub generate: GenerationOptions,
    pub package: PackageOptions,
    pub post: PostOptions,
    pub obs_version: ObsVersion,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ObsVersion {
    pub version_str: String,
    pub version_major: u8,
    pub version_minor: u8,
    pub version_patch: u8,
    pub beta: u8,
    pub rc: u8,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct EnvOptions {
    #[serde(default = "get_default_branch")]
    pub branch: String,
    pub input_dir: PathBuf,
    pub output_dir: PathBuf,
    pub previous_dir: PathBuf,
    // Todo replace 7zip, OpenSSL, Signtool
    // signtool_opts: Option<Vec<String>>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct PreparationOptions {
    pub copy: CopyOptions,
    pub codesign: CodesignOptions,
    pub strip_pdbs: StripPDBOptions,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct CopyOptions {
    pub excludes: Vec<String>,
    pub overrides: Vec<(String, String)>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct CodesignOptions {
    pub skip_sign: bool,
    pub sign_name: String,
    pub sign_digest: String,
    pub sign_ts_serv: String,
    pub sign_exts: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct StripPDBOptions {
    pub pdbcopy_path: PathBuf,
    pub exclude: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct GenerationOptions {
    // patch_type: String,
    pub removed_files: Vec<String>,
    pub packages: Vec<ManifestPackageOptions>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ManifestPackageOptions {
    pub name: String,
    pub include_files: Option<Vec<String>>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct PackageOptions {
    pub installer: InstallerOptions,
    pub zip: ZipOptions,
    pub updater: UpdaterOptions,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct InstallerOptions {
    pub makensis_path: PathBuf,
    pub nsis_script: String,
    pub name: String,
    pub skip_sign: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ZipOptions {
    pub name: String,
    pub pdb_name: String,
    pub skip_for_prerelease: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct UpdaterOptions {
    pub pandoc_path: PathBuf,
    pub sign: bool,
    pub updater_path: PathBuf,
    pub private_key: PathBuf,
    pub private_key_env: String,
    pub skip_for_prerelease: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct PostOptions {
    pub move_to_old: bool,
}

impl Config {
    pub fn set_version(&mut self, version_string: &String, beta_num: u8, rc_num: u8) {
        let ver_parsed = parse_version(version_string);
        self.obs_version.version_str = version_string.to_owned();
        self.obs_version.version_major = ver_parsed.0;
        self.obs_version.version_minor = ver_parsed.1;
        self.obs_version.version_patch = ver_parsed.2;
        self.obs_version.beta = beta_num;
        self.obs_version.rc = rc_num;
    }

    pub fn set_dirs(&mut self, input: Option<PathBuf>, output: Option<PathBuf>, previous: Option<PathBuf>) {
        if let Some(input) = input {
            self.env.input_dir = input;
        }
        if let Some(output) = output {
            self.env.output_dir = output;
        }
        if let Some(previous) = previous {
            self.env.previous_dir = previous;
        }
    }

    pub fn from_file(path: &Path) -> Config {
        let config: Option<Config> = fs::read_to_string(path)
            .ok()
            .and_then(|fc| toml::from_str(fc.as_str()).ok());

        if config.is_none() {
            panic!("Failed to parse config!")
        }

        config.unwrap()
    }
}
