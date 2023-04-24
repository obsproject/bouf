use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{bail, Result};
use log::warn;
use serde::{Deserialize, Deserializer};
use toml;

use crate::models::args::MainArgs;
use crate::utils::misc;
use crate::utils::sign::Signer;

fn get_default_branch() -> String {
    String::from("stable")
}
fn get_default_log_level() -> String {
    String::from("info")
}
fn get_7z_bin() -> PathBuf {
    PathBuf::from("7z")
}
fn get_makensis_bin() -> PathBuf {
    PathBuf::from("makensis")
}
fn get_pandoc_bin() -> PathBuf {
    PathBuf::from("pandoc")
}
fn get_pdbcopy_bin() -> PathBuf {
    PathBuf::from("pdbcopy")
}
fn get_compression_default() -> bool {
    true
}
fn get_always_copied() -> Vec<String> {
    vec![
        "obs64".to_string(),
        "obspython".to_string(),
        "obslua".to_string(),
        "obs-frontend-api".to_string(),
        "obs.dll".to_string(),
        "obs.pdb".to_string(),
    ]
}
fn get_signed_exts() -> Vec<String> {
    vec!["exe".to_string(), "dll".to_string(), "pyd".to_string()]
}
fn get_default_zip_name() -> String {
    String::from("OBS-Studio-{version}.zip")
}
fn get_default_pdb_zip_name() -> String {
    String::from("OBS-Studio-{version}-pdbs.zip")
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Config {
    // Sections
    pub general: GeneralOptions,
    pub env: EnvOptions,
    pub prepare: PreparationOptions,
    pub generate: GenerationOptions,
    pub package: PackageOptions,
    pub post: PostOptions,
    pub obs_version: ObsVersion,
}

#[derive(Deserialize, Default, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct ObsVersion {
    pub commit: String,
    pub version_str: String,
    pub version_major: u8,
    pub version_minor: u8,
    pub version_patch: u8,
    pub beta: u8,
    pub rc: u8,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct GeneralOptions {
    #[serde(default = "get_default_branch")]
    pub branch: String,
    #[serde(default = "get_default_log_level")]
    pub log_level: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct EnvOptions {
    pub input_dir: PathBuf,
    pub output_dir: PathBuf,
    pub previous_dir: PathBuf,
    // Tool paths
    #[serde(default = "get_7z_bin")]
    pub sevenzip_path: PathBuf,
    #[serde(default = "get_makensis_bin")]
    pub makensis_path: PathBuf,
    #[serde(default = "get_pandoc_bin")]
    pub pandoc_path: PathBuf,
    #[serde(default = "get_pdbcopy_bin")]
    pub pdbcopy_path: PathBuf,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct PreparationOptions {
    pub empty_output_dir: bool,
    pub copy: CopyOptions,
    pub codesign: CodesignOptions,
    pub strip_pdbs: StripPDBOptions,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct CopyOptions {
    pub excludes: Vec<String>,
    pub never_copy: Vec<String>,
    #[serde(default = "get_always_copied")]
    pub always_copy: Vec<String>,
    pub overrides: Vec<(String, String)>,
    pub overrides_sign: Vec<(String, String)>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct CodesignOptions {
    pub skip_sign: bool,
    pub sign_name: String,
    pub sign_digest: String,
    pub sign_ts_serv: String,
    #[serde(default = "get_signed_exts")]
    pub sign_exts: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct StripPDBOptions {
    pub exclude: Vec<String>,
    pub skip_for_prerelease: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct GenerationOptions {
    #[serde(deserialize_with = "deserialize_patch_type")]
    pub patch_type: PatchType,
    pub skip_for_prerelease: bool,
    #[serde(default = "get_compression_default")]
    pub compress_files: bool,
    pub removed_files: Vec<String>,
    pub exclude_from_parallel: Vec<String>,
    pub exclude_from_removal: Vec<String>,
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
    pub nsis_script: PathBuf,
    pub name: String,
    pub skip_sign: bool,
    pub skip: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ZipOptions {
    #[serde(default = "get_default_zip_name")]
    pub name: String,
    #[serde(default = "get_default_pdb_zip_name")]
    pub pdb_name: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct UpdaterOptions {
    pub skip_sign: bool,
    pub pretty_json: bool,
    pub notes_file: PathBuf,
    pub updater_path: PathBuf,
    pub private_key: Option<PathBuf>,
    pub vc_redist_path: PathBuf,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct PostOptions {
    pub copy_to_old: bool,
}

impl From<&ObsVersion> for u32 {
    fn from(obsver: &ObsVersion) -> Self {
        let mut ver = 0;
        ver += (obsver.version_major as u32) << 24;
        ver += (obsver.version_minor as u32) << 16;
        ver += (obsver.version_patch as u32) << 8;
        ver += (obsver.rc as u32) << 4;
        ver += obsver.beta as u32;

        ver
    }
}

impl PartialOrd for ObsVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let self_ver: u32 = self.into();
        let other_ver: u32 = other.into();

        Some(self_ver.cmp(&other_ver))
    }
}

impl Config {
    pub fn set_version(&mut self, version_string: &String, beta_num: u8, rc_num: u8) -> Result<()> {
        self.obs_version = misc::parse_version(version_string)?;

        if beta_num > 0 {
            self.obs_version.beta = beta_num
        } else if rc_num > 0 {
            self.obs_version.rc = rc_num
        }

        Ok(())
    }

    pub fn apply_args(&mut self, args: &MainArgs) -> Result<()> {
        self.set_version(
            &args.version,
            args.beta.unwrap_or_default(),
            args.rc.unwrap_or_default(),
        )?;

        if let Some(input) = &args.input {
            self.env.input_dir = input.clone();
        }
        if let Some(output) = &args.output {
            self.env.output_dir = output.clone();
        }
        if let Some(previous) = &args.previous {
            self.env.previous_dir = previous.clone();
        }
        if let Some(branch) = &args.branch {
            self.general.branch = branch.to_owned();
        }
        if let Some(commit) = &args.commit {
            self.obs_version.commit = commit.replace('g', "");
        }

        self.prepare.empty_output_dir = args.clear_output;
        self.prepare.codesign.skip_sign = args.skip_codesigning || self.prepare.codesign.skip_sign;
        self.package.installer.skip_sign = args.skip_codesigning || self.package.installer.skip_sign;
        self.package.installer.skip = args.skip_installer || self.package.installer.skip;
        self.package.updater.skip_sign = args.skip_manifest_signing || self.package.updater.skip_sign;

        if let Some(privkey) = &args.private_key {
            self.package.updater.private_key = Some(privkey.to_owned());
        }
        if let Some(notes_file) = &args.notes_file {
            self.package.updater.notes_file = fs::canonicalize(notes_file)?;
        }

        self.validate(false)
    }

    pub fn validate(&mut self, deltas_only: bool) -> Result<()> {
        // Output folder cannot be checked as it may not exist yet
        match fs::canonicalize(&self.env.input_dir) {
            Ok(res) => self.env.input_dir = res,
            Err(e) => bail!("Input dir error: {}", e),
        }

        // Ensure previous folder and subdirectories exist
        match fs::canonicalize(&self.env.previous_dir) {
            Ok(res) => {
                fs::create_dir_all(res.join("builds"))?;
                fs::create_dir_all(res.join("pdbs"))?;
                self.env.previous_dir = res;
            }
            Err(e) => bail!("Previous dir error: {}", e),
        }

        // This function will just return the original path if it doesn't succeed.
        self.env.output_dir = misc::recursive_canonicalize(&self.env.output_dir);

        // Create default package if none exist
        if self.generate.packages.is_empty() || !self.generate.packages.iter().any(|f| f.include_files.is_none()) {
            self.generate.packages.push(ManifestPackageOptions {
                name: "core".to_string(),
                ..Default::default()
            });
        }

        // This is all we care about if we're only generating deltas
        if deltas_only {
            return Ok(());
        }

        // Check file paths (for binaries, also check if they are in %PATH%)
        misc::check_binary_path(&mut self.env.pdbcopy_path)?;
        misc::check_binary_path(&mut self.env.makensis_path)?;
        misc::check_binary_path(&mut self.env.sevenzip_path)?;
        misc::check_binary_path(&mut self.env.pandoc_path)?;

        // Check if private key is set correctly (if signing is enabled)
        if !self.package.updater.skip_sign {
            if let Err(e) = Signer::check_key(self.package.updater.private_key.as_ref()) {
                bail!("Failed loading private key: {}", e)
            }
        }

        // Check if codesigning parameters are set (if enabled)
        #[cfg(windows)]
        if !self.prepare.codesign.skip_sign
            && (self.prepare.codesign.sign_name.is_empty()
                || self.prepare.codesign.sign_digest.is_empty()
                || self.prepare.codesign.sign_ts_serv.is_empty()
                || self.prepare.codesign.sign_exts.is_empty())
        {
            bail!("Codesigning settings are incomplete!")
        }

        if !self.prepare.copy.excludes.is_empty() {
            warn!("\"excludes\" is deprecated in favour of \"never_copy\"");
            self.prepare.copy.never_copy.append(&mut self.prepare.copy.excludes);
        }

        if !self.prepare.copy.overrides_sign.is_empty() {
            warn!("\"overrides_sign\" is deprecated in favour of \"overrides\"");
            self.prepare
                .copy
                .overrides
                .append(&mut self.prepare.copy.overrides_sign);
        }

        // Check that NSIS script exists if installer not skipped
        if !self.package.installer.skip && !self.package.installer.nsis_script.exists() {
            bail!("NSIS script does not exist!")
        }

        // Check that notes and vc redist files exists
        if !self.package.updater.vc_redist_path.exists() {
            bail!(
                "VC Redist file not found at \"{}\"!",
                self.package.updater.vc_redist_path.to_str().unwrap_or("<INVALID PATH>")
            )
        }

        if !self.package.updater.notes_file.exists() {
            bail!(
                "Release notes file not found at \"{}\"!",
                self.package.updater.notes_file.to_str().unwrap_or("<INVALID PATH>")
            )
        }

        Ok(())
    }

    pub fn from_file(path: &Path) -> Result<Config> {
        let config_str = fs::read_to_string(path)?;
        let config = toml::from_str::<Config>(config_str.as_str())?;

        Ok(config)
    }
}

#[derive(Debug, PartialEq, Eq, Default, Deserialize)]
pub enum PatchType {
    BsdiffLzma,
    #[default]
    Zstd,
}

impl FromStr for PatchType {
    type Err = ();

    fn from_str(input: &str) -> Result<PatchType, Self::Err> {
        match input {
            "bsdiff_lzma" => Ok(PatchType::BsdiffLzma),
            "zstd" => Ok(PatchType::Zstd),
            _ => Err(()),
        }
    }
}

fn deserialize_patch_type<'de, D>(deserializer: D) -> Result<PatchType, D::Error>
where
    D: Deserializer<'de>,
{
    let buf = String::deserialize(deserializer)?;

    match PatchType::from_str(&buf) {
        Ok(val) => Ok(val),
        Err(_) => Err(serde::de::Error::custom("Failed reading patch_type")),
    }
}
