use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Result};
use hashbrown::HashSet;
use walkdir::{DirEntry, WalkDir};

use crate::models::config::Config;
use crate::utils::codesign::sign;
use crate::utils::misc;

pub struct Preparator<'a> {
    config: &'a Config,
    input_path: PathBuf,
    install_path: PathBuf,
    pdbs_path: PathBuf,
}

impl<'a> Preparator<'a> {
    pub fn init(conf: &'a Config) -> Self {
        Self {
            config: conf,
            input_path: misc::normalize_path(&conf.env.input_dir),
            install_path: misc::normalize_path(&conf.env.output_dir.join("install")),
            pdbs_path: misc::normalize_path(&conf.env.output_dir.join("pdbs")),
        }
    }

    /// Create/clear output directory
    fn ensure_output_dir(&self) -> Result<()> {
        if self.install_path.exists() && !self.install_path.read_dir()?.next().is_none() {
            if !self.config.prepare.empty_output_dir {
                bail!("Folder not empty");
            }
            println!("[!] Deleting previous output dir...");
            std::fs::remove_dir_all(&self.install_path)?;
        }

        std::fs::create_dir_all(&self.install_path)?;
        Ok(())
    }

    /// Copy input files to "install" dir
    fn copy(&self) -> Result<()> {
        let mut overrides: HashSet<&String> = HashSet::new();
        // Convert to hash set for fast lookup
        self.config.prepare.copy.overrides.iter().for_each(|(obs_path, _)| {
            overrides.insert(obs_path);
        });

        println!(
            "[+] Copying build from \"{}\" to \"{}\"...",
            self.input_path.display(),
            self.install_path.display()
        );
        std::fs::create_dir_all(&self.install_path)?;

        // Walk dir, honor overrides where necessary
        for file in WalkDir::new(&self.input_path)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| !e.file_type().is_dir())
        {
            let file: DirEntry = file;
            // Get a path relative to the input directory for lookup/copy path
            let relative_path = file.path().strip_prefix(&self.input_path).unwrap().to_str().unwrap();
            let relative_path_str = String::from(relative_path).replace("\\", "/");
            // Check against overrides
            if overrides.contains(&relative_path_str) {
                continue;
            }
            // Check relative path against excludes
            if self
                .config
                .prepare
                .copy
                .excludes
                .iter()
                .any(|x| relative_path_str.contains(x))
            {
                continue;
            }
            let file_path = self.install_path.join(relative_path);
            // Ensure dir structure exists
            if let Some(_parent) = file_path.parent() {
                fs::create_dir_all(_parent)?;
            }
            fs::copy(file.path(), file_path)?;
        }

        // Copy override files over
        for (ins_path, ovr_path) in &self.config.prepare.copy.overrides {
            if !fs::metadata(ovr_path).is_ok() {
                bail!("Override file \"{}\" does not exist!", ovr_path)
            }

            let full_path = self.install_path.join(ins_path);
            if let Some(_parent) = full_path.parent() {
                fs::create_dir_all(_parent)?;
            }
            fs::copy(ovr_path, full_path)?;
        }

        Ok(())
    }

    /// Move PDBs (except excluded) to separate dir, then strip remaining ones
    fn strip_pdbs(&self) -> Result<()> {
        let opts = &self.config.prepare.strip_pdbs;
        let copy_opts = &self.config.prepare.copy;

        println!(
            "[+] Copying/stripping PDBs from \"{}\" to \"{}\"...",
            self.install_path.display(),
            self.pdbs_path.display()
        );

        for file in WalkDir::new(&self.install_path)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| !e.file_type().is_dir())
        {
            let file: DirEntry = file;
            let relative_path = file.path().strip_prefix(&self.install_path).unwrap().to_str().unwrap();
            if !relative_path.ends_with(".pdb") {
                continue;
            }
            let relative_path_str = String::from(relative_path).replace("\\", "/");
            let new_path = self.pdbs_path.join(relative_path);
            if let Some(_parent) = new_path.parent() {
                fs::create_dir_all(_parent)?;
            }
            // Skip files excluded or that were overrides
            if opts.exclude.iter().any(|x| relative_path_str.contains(x))
                || copy_opts.overrides.iter().any(|(p, _)| relative_path_str == *p)
            {
                fs::copy(file.path(), &new_path)?;
                continue;
            }

            fs::rename(file.path(), &new_path)?;

            // Finally, run PDBCopy
            Command::new(&self.config.env.pdbcopy_path)
                .args([new_path.as_os_str(), file.path().as_os_str(), OsStr::new("-p")])
                .output()
                .expect("failed to run pdbcopy");
        }
        Ok(())
    }

    /// Sign all eligible files in a folder using Signtool
    #[cfg(windows)]
    fn codesign(&self) -> Result<()> {
        if self.config.prepare.codesign.skip_sign {
            return Ok(());
        }

        let exts = &self.config.prepare.codesign.sign_exts;
        let overrides = &self.config.prepare.copy.overrides;

        println!("[+] Signing files in \"{}\"", self.install_path.display());
        let mut to_sign: Vec<PathBuf> = Vec::new();

        for file in WalkDir::new(&self.install_path)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| !e.file_type().is_dir())
        {
            let file: DirEntry = file;
            let relative_path = file.path().to_str().unwrap();

            if !exts.iter().any(|x| relative_path.ends_with(x.as_str())) {
                continue;
            }
            // Do not re-sign files that were copied
            let relative_path_str = String::from(relative_path).replace("\\", "/");
            if overrides.iter().any(|(p, _)| relative_path_str == *p) {
                continue;
            }
            to_sign.push(file.path().canonicalize()?)
        }
        sign(to_sign, &self.config.prepare.codesign)?;

        Ok(())
    }

    #[cfg(unix)]
    fn codesign(&self) -> Result<()> {
        println!("Codesigning is not (yet) supported on this platform.");
        Ok(())
    }

    pub fn run(self) -> Result<()> {
        self.ensure_output_dir()?;
        self.copy()?;
        self.codesign()?;
        self.strip_pdbs()?;

        Ok(())
    }
}
