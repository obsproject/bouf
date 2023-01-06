use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Result};
use hashbrown::HashSet;
use walkdir::{DirEntry, WalkDir};

#[cfg(windows)]
use crate::utils::codesign::sign;

use crate::models::config::{Config, CopyOptions, ObsVersion};
use crate::utils::misc;
use crate::utils::misc::parse_version;

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
        let out_dir = &self.config.env.output_dir;
        if out_dir.exists() && out_dir.read_dir()?.next().is_some() {
            if !self.config.prepare.empty_output_dir {
                bail!("Output folder not empty!");
            }
            println!("[!] Deleting previous output dir...");
            std::fs::remove_dir_all(out_dir)?;
        }

        std::fs::create_dir_all(out_dir)?;
        Ok(())
    }

    /// Copy input files to "install" dir
    fn copy(&self) -> Result<()> {
        let copy_opts = &self.config.prepare.copy;

        println!(
            "[+] Copying build from \"{}\" to \"{}\"...",
            self.input_path.display(),
            self.install_path.display()
        );

        copy_files(copy_opts, &self.input_path, &self.install_path, false)?;

        // Copy override files over
        for (ins_path, ovr_path) in [copy_opts.overrides.as_slice(), copy_opts.overrides_sign.as_slice()].concat() {
            if fs::metadata(&ovr_path).is_err() {
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

    /// Copy excluded files from previous build and PDB directories
    fn copy_previous(&self) -> Result<()> {
        let copy_opts = &self.config.prepare.copy;

        let is_prerelease = self.config.obs_version.rc > 0
            || self.config.obs_version.beta > 0
            || !self.config.obs_version.commit.is_empty();

        // Iterate over old builds to find the latest one
        let mut ver_str = String::from("0.0.0");
        let mut latest_ver: ObsVersion = parse_version(&ver_str)?;
        for item in fs::read_dir(self.config.env.previous_dir.join("builds"))?.flatten() {
            let meta = item.metadata()?;
            if !meta.is_dir() {
                continue;
            }
            let name = String::from(item.file_name().to_str().unwrap());
            let ver = parse_version(&name)?;

            // Do not pull files from pre-release builds unless we're doing a pre-release build
            if !is_prerelease && (ver.beta > 0 || ver.rc > 0 || !ver.commit.is_empty()) {
                continue;
            }

            if ver > latest_ver && ver < self.config.obs_version {
                latest_ver = ver;
                ver_str = name;
            }
        }

        if latest_ver.version_major == 0 && latest_ver.version_minor == 0 && latest_ver.version_patch == 0 {
            bail!("No valid previous version found!")
        }

        let build_path: PathBuf = self.config.env.previous_dir.join("builds").join(&ver_str);
        let pdb_path: PathBuf = self.config.env.previous_dir.join("pdbs").join(&ver_str);

        if !build_path.exists() {
            bail!("Previous build path \"{}\" does not exist!", pdb_path.display());
        } else if !pdb_path.exists() {
            bail!("Previous PDB path \"{}\" does not exist!", pdb_path.display());
        }

        // Copy binaries
        println!(
            "[+] Copying old build files from \"{}\" to \"{}\"...",
            build_path.display(),
            self.install_path.display()
        );
        copy_files(copy_opts, &build_path, &self.install_path, true)?;

        // Copy unstripped PDBs
        println!(
            "[+] Copying old PDB files from \"{}\" to \"{}\"...",
            pdb_path.display(),
            self.pdbs_path.display()
        );
        copy_files(copy_opts, &pdb_path, &self.pdbs_path, true)?;

        Ok(())
    }

    /// Move PDBs (except excluded) to separate dir, then strip remaining ones
    fn strip_pdbs(&self) -> Result<()> {
        let opts = &self.config.prepare.strip_pdbs;
        let copy_opts = &self.config.prepare.copy;

        let is_prerelease = self.config.obs_version.rc > 0
            || self.config.obs_version.beta > 0
            || !self.config.obs_version.commit.is_empty();

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
            let relative_path_str = String::from(relative_path).replace('\\', "/");
            let new_path = self.pdbs_path.join(relative_path);
            if let Some(_parent) = new_path.parent() {
                fs::create_dir_all(_parent)?;
            }
            // Skip files excluded or that were overrides, also do not strip for betas if enabled
            if (opts.skip_for_prerelease && is_prerelease)
                || opts.exclude.iter().any(|x| relative_path_str.contains(x))
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
            let relative_path_str = String::from(relative_path).replace('\\', "/");
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
        if !self.config.prepare.copy.include.is_empty() || !self.config.prepare.copy.exclude.is_empty() {
            self.copy_previous()?;
        }

        Ok(())
    }
}

fn copy_files(opts: &CopyOptions, input: &PathBuf, output: &Path, copying_old: bool) -> Result<()> {
    // Include filter needs to be inverted when copying old files
    let includes = if !copying_old { &opts.include } else { &opts.exclude };

    // Concatenate all exclude filters
    let mut excludes: HashSet<&String> = HashSet::new();
    // Config excludes
    excludes.extend(opts.excludes.iter());
    // Simple filters, again inverted for old build copy
    if !copying_old {
        excludes.extend(opts.exclude.iter());
    } else {
        excludes.extend(opts.include.iter());
    }
    // Overrides are also excludes
    opts.overrides.iter().for_each(|(obs_path, _)| {
        excludes.insert(obs_path);
    });
    opts.overrides_sign.iter().for_each(|(obs_path, _)| {
        excludes.insert(obs_path);
    });

    std::fs::create_dir_all(output)?;

    for file in WalkDir::new(input)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| !e.file_type().is_dir())
    {
        let file: DirEntry = file;
        // Get a path relative to the input directory for lookup/copy path and
        // replace \ with / since we use Unix-style paths in most cases
        let relative_path = file.path().strip_prefix(input).unwrap();
        let relative_path_str = String::from(relative_path.to_str().unwrap()).replace('\\', "/");

        if !relative_path.starts_with("bin")
            && !relative_path.starts_with("data")
            && !relative_path.starts_with("obs-plugins")
        {
            continue;
        }
        if !includes.is_empty() && !includes.iter().any(|f| relative_path_str.contains(f)) {
            continue;
        }
        if excludes.iter().any(|f| relative_path_str.contains(*f)) {
            continue;
        }

        let file_path = output.join(relative_path);
        // Ensure dir structure exists
        if let Some(_parent) = file_path.parent() {
            fs::create_dir_all(_parent)?;
        }
        fs::copy(file.path(), file_path)?;
    }

    Ok(())
}
