use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{anyhow, Result};
use log::error;
#[cfg(windows)]
use log::info;

#[cfg(windows)]
use crate::utils::codesign::sign;

use crate::models::config::{Config, EnvOptions};
use crate::models::manifest::Manifest;
use crate::utils::hash::hash_file;
use crate::utils::misc;

#[allow(dead_code)]
pub struct Packaging<'a> {
    config: &'a Config,
    short_version: String,
    #[allow(unused_variables)]
    tag_version: String,
}

impl<'a> Packaging<'a> {
    pub fn init(conf: &'a Config) -> Self {
        Self {
            config: conf,
            short_version: misc::get_filename_version(&conf.obs_version, true),
            tag_version: misc::get_filename_version(&conf.obs_version, false),
        }
    }

    #[cfg(windows)]
    #[allow(clippy::uninlined_format_args)]
    pub fn run_nsis(&self) -> Result<()> {
        // ToDo make installer name more configurable
        let nsis_script = self.config.package.installer.nsis_script.canonicalize()?;
        // The build dir is the "install" subfolder in the output dir
        let build_dir = self.config.env.output_dir.join("install").canonicalize()?;
        let mut build_dir_str = build_dir.into_os_string().into_string().unwrap();
        // Sanitise build dir string for NSIS
        if build_dir_str.starts_with('\\') {
            build_dir_str = build_dir_str.strip_prefix("\\\\?\\").unwrap().to_string();
        }

        let args: Vec<OsString> = vec![
            format!("/DTAGVERSION={}", self.tag_version).into(),
            format!("/DAPPVERSION={}", self.config.obs_version.version_str).into(),
            format!("/DSHORTVERSION={}", self.short_version).into(),
            format!("/DBUILDDIR={}", build_dir_str).into(),
            nsis_script.into_os_string(),
        ];

        info!(" => Running NSIS...");
        let output = Command::new(&self.config.env.makensis_path).args(args).output()?;

        if !output.status.success() {
            error!("MakeNSIS returned non-success status: {}", output.status);
            std::io::stdout().write_all(&output.stdout)?;
            std::io::stderr().write_all(&output.stderr)?;

            Err(anyhow!("MakeNSIS failed (see stdout/stderr for details)"))
        } else {
            info!("NSIS completed successfully!");

            if !self.config.package.installer.skip_sign {
                self.sign_installer()?;
                info!("Installer signed successfully!");
            }

            Ok(())
        }
    }

    #[cfg(unix)]
    pub fn run_nsis(&self) -> Result<()> {
        use log::warn;
        warn!("Creating an installer is not (yet) supported on this platform.");

        Ok(())
    }

    #[cfg(windows)]
    fn sign_installer(&self) -> Result<()> {
        let filename = format!("OBS-Studio-{}-Full-Installer-x64.exe", self.short_version);
        let path = self.config.env.output_dir.join(filename).canonicalize()?;

        info!("Signing installer file \"{}\"", path.display());
        let files: Vec<PathBuf> = vec![path];
        sign(files, &self.config.prepare.codesign)?;

        Ok(())
    }

    pub fn create_zips(&self) -> Result<()> {
        let zip_name = self.config.package.zip.name.replace("{version}", &self.short_version);
        let pdb_zip_name = self
            .config
            .package
            .zip
            .pdb_name
            .replace("{version}", &self.short_version);

        let obs_path = self.config.env.output_dir.join("install/*");
        let pdb_path = self.config.env.output_dir.join("pdbs/*");
        let obs_zip_path = self.config.env.output_dir.join(zip_name);
        let pdb_zip_path = self.config.env.output_dir.join(pdb_zip_name);

        run_sevenzip(&self.config.env.sevenzip_path, &obs_path, &obs_zip_path)?;
        run_sevenzip(&self.config.env.sevenzip_path, &pdb_path, &pdb_zip_path)?;

        Ok(())
    }

    pub fn finalise_manifest(&self, manifest: &mut Manifest) -> Result<PathBuf> {
        let branch = &self.config.general.branch;

        let manifest_filename = if branch.is_empty() || branch == "stable" {
            "manifest.json".to_string()
        } else {
            format!("manifest_{branch}.json")
        };

        let manifest_path = self.config.env.output_dir.join(manifest_filename);
        let notes_path = self.config.env.output_dir.join("notes.rst");

        // Add VC hash
        let hash = hash_file(&self.config.package.updater.vc_redist_path);
        manifest.vc2019_redist_x64 = hash.hash;

        // Add notes and copy them to output
        manifest.notes = run_pandoc(&self.config.package.updater.notes_file, &self.config.env)?;
        std::fs::copy(&self.config.package.updater.notes_file, notes_path)?;

        manifest.to_file(&manifest_path, self.config.package.updater.pretty_json)?;

        Ok(manifest_path)
    }
}

fn run_sevenzip(sevenzip: &PathBuf, in_path: &PathBuf, out_path: &PathBuf) -> Result<()> {
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
        error!("7-zip returned non-success status: {}", output.status);
        std::io::stdout().write_all(&output.stdout)?;
        std::io::stderr().write_all(&output.stderr)?;

        Err(anyhow!("7-zip failed (see stdout/stderr for details)"))
    } else {
        Ok(())
    }
}

fn run_pandoc(path: &PathBuf, env: &EnvOptions) -> Result<String> {
    let args: Vec<OsString> = vec![
        "--from".into(),
        "markdown".into(),
        "--to".into(),
        "html".into(),
        path.to_owned().into_os_string(),
    ];

    let output = Command::new(&env.pandoc_path).args(args).output()?;

    if !output.status.success() {
        error!("pandoc returned non-success status: {}", output.status);
        std::io::stdout().write_all(&output.stdout)?;
        std::io::stderr().write_all(&output.stderr)?;
        Err(anyhow!("pandoc failed (see stdout/stderr for details)"))
    } else {
        Ok(String::from_utf8(output.stdout)?)
    }
}
