use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use toml;

use crate::models::args::MainArgs;
use crate::models::errors::SomeError;
use crate::utils::{misc, sign};

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
pub struct EnvOptions {
    #[serde(default = "get_default_branch")]
    pub branch: String,
    pub input_dir: PathBuf,
    pub output_dir: PathBuf,
    pub previous_dir: PathBuf,
    // Tool paths
    pub sevenzip_path: PathBuf,
    pub makensis_path: PathBuf,
    pub pandoc_path: PathBuf,
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
    pub exclude: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct GenerationOptions {
    // patch_type: String,
    pub skip_for_prerelease: bool,
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
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ZipOptions {
    pub name: String,
    pub pdb_name: String,
    pub skip_pdbs_for_prerelease: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct UpdaterOptions {
    pub skip_sign: bool,
    pub pretty_json: bool,
    pub notes_files: PathBuf,
    pub updater_path: PathBuf,
    pub private_key: Option<PathBuf>,
    pub vc_redist_path: PathBuf,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct PostOptions {
    pub copy_to_old: bool,
}

impl Config {
    pub fn set_version(&mut self, version_string: &String, beta_num: u8, rc_num: u8) {
        let ver_parsed = misc::parse_version(version_string);
        self.obs_version.version_str = version_string.split("-").next().unwrap().to_string();
        self.obs_version.version_major = ver_parsed.0;
        self.obs_version.version_minor = ver_parsed.1;
        self.obs_version.version_patch = ver_parsed.2;
        self.obs_version.beta = if beta_num > 0 { beta_num } else { ver_parsed.3 };
        self.obs_version.rc = if rc_num > 0 { rc_num } else { ver_parsed.4 };
    }

    pub fn apply_args(&mut self, args: &MainArgs) {
        self.set_version(
            &args.version,
            args.beta.unwrap_or_default(),
            args.rc.unwrap_or_default(),
        );
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
            self.env.branch = branch.to_owned();
        }
        if let Some(commit) = &args.commit {
            self.obs_version.commit = commit.replace("g", "");
        }

        self.prepare.empty_output_dir = args.clear_output;
        self.prepare.codesign.skip_sign = args.skip_codesigning || self.prepare.codesign.skip_sign;
        self.package.installer.skip_sign = args.skip_codesigning || self.package.installer.skip_sign;
        self.package.updater.skip_sign = args.skip_manifest_signing || self.package.updater.skip_sign;
        if let Some(privkey) = &args.private_key {
            self.package.updater.private_key = Some(privkey.to_owned());
        }
        // Todo remaining args
    }

    pub fn validate(&mut self, check_binaries: bool, check_paths: bool) -> Result<(), SomeError> {
        // Check file paths (for binaries, also check if they are in %PATH%)
        if check_binaries {
            misc::check_binary_path(&mut self.env.pdbcopy_path)?;
            misc::check_binary_path(&mut self.env.makensis_path)?;
            misc::check_binary_path(&mut self.env.sevenzip_path)?;
            misc::check_binary_path(&mut self.env.pandoc_path)?;
        }
        // Check if private key is set correctly (if signing is enabled)
        if !self.package.updater.skip_sign {
            if let Err(e) = Signer::check_key(self.package.updater.private_key.as_ref()) {
                return Err(SomeError(format!("Failed loading private key: {}", e)));
            }
        }
        // Check if codesigning parameters are set (if enabled)
        if !self.prepare.codesign.skip_sign {
            // ToDo
        }
        // Check file/directory paths
        if check_paths {
            // Output folder cannot be checked as it may not exist yet
            match fs::canonicalize(&self.env.input_dir) {
                Ok(res) => self.env.input_dir = res,
                Err(e) => return Err(SomeError(format!("Input dir error: {}", e))),
            }
            match fs::canonicalize(&self.env.previous_dir) {
                Ok(res) => self.env.previous_dir = res,
                Err(e) => return Err(SomeError(format!("Previous dir error: {}", e))),
            }
            // This function will just return the original path if it doesn't succeed.
            self.env.output_dir = misc::recursive_canonicalize(&self.env.output_dir);
            // ToDo Check other files (nsis script, updater, vcredist)
        }
        // Check that config defines at least one package
        if self.generate.packages.len() < 1 {
            return Err(SomeError("No packages defined in config!".into()));
        }
        // Check if a manifest package is defined that does not have any filters
        if !self.generate.packages.iter().any(|f| f.include_files.is_none()) {
            return Err(SomeError("No catchall package exists in conifg!".into()));
        }

        Ok(())
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
