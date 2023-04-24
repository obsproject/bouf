use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Result};
use hashbrown::HashSet;
use log::{debug, info, warn};
use walkdir::{DirEntry, WalkDir};

#[cfg(windows)]
use crate::utils::codesign::sign;

use crate::models::config::{Config, CopyOptions, ObsVersion};
use crate::utils::hash::get_dir_code_hashes;
use crate::utils::misc;
use crate::utils::misc::parse_version;

const BINARY_EXTS: [&str; 4] = ["exe", "pdb", "pyd", "dll"];

pub struct Preparator<'a> {
    config: &'a Config,
    input_path: PathBuf,
    install_path: PathBuf,
    pdbs_path: PathBuf,
    prev_bin_path: Option<PathBuf>,
    prev_pdb_path: Option<PathBuf>,
    exclude: HashSet<String>,
}

impl<'a> Preparator<'a> {
    pub fn init(conf: &'a Config) -> Self {
        let input = misc::normalize_path(&conf.env.input_dir);
        let install = misc::normalize_path(&conf.env.output_dir.join("install"));
        let pdbs = misc::normalize_path(&conf.env.output_dir.join("pdbs"));

        Self {
            config: conf,
            input_path: input,
            install_path: install,
            pdbs_path: pdbs,
            prev_bin_path: None,
            prev_pdb_path: None,
            exclude: HashSet::new(),
        }
    }

    /// Create/clear output directory
    fn ensure_output_dir(&self) -> Result<()> {
        let out_dir = &self.config.env.output_dir;
        if out_dir.exists() && out_dir.read_dir()?.next().is_some() {
            if !self.config.prepare.empty_output_dir {
                bail!("Output folder not empty!");
            }
            warn!("Deleting previous output dir...");
            fs::remove_dir_all(out_dir)?;
        }

        fs::create_dir_all(out_dir)?;
        Ok(())
    }

    /// Copy input files to "install" dir
    fn copy(&self) -> Result<()> {
        let copy_opts = &self.config.prepare.copy;

        info!(
            "Copying build from \"{}\" to \"{}\"...",
            self.input_path.display(),
            self.install_path.display()
        );

        copy_files(copy_opts, &self.input_path, &self.install_path, false, &self.exclude)?;

        // Copy override files over
        for (ins_path, ovr_path) in copy_opts.overrides.as_slice() {
            if fs::metadata(ovr_path).is_err() {
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
    fn find_previous(&mut self) -> Result<()> {
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
            bail!("Previous build path \"{}\" does not exist!", build_path.display());
        } else if !pdb_path.exists() {
            bail!("Previous PDB path \"{}\" does not exist!", pdb_path.display());
        }

        self.prev_pdb_path = Some(pdb_path);
        self.prev_bin_path = Some(build_path);

        Ok(())
    }

    fn copy_previous(&self) -> Result<()> {
        if self.prev_bin_path.is_none() {
            return Ok(());
        }

        let prev_bin_path = self.prev_bin_path.as_ref().unwrap();
        let prev_pdb_path = self.prev_pdb_path.as_ref().unwrap();
        let copy_opts = &self.config.prepare.copy;
        // Copy binaries
        info!(
            "Copying old build files from \"{}\" to \"{}\"...",
            prev_bin_path.display(),
            self.install_path.display()
        );
        copy_files(copy_opts, prev_bin_path, &self.install_path, true, &self.exclude)?;

        // Copy unstripped PDBs
        info!(
            "Copying old PDB files from \"{}\" to \"{}\"...",
            prev_pdb_path.display(),
            self.pdbs_path.display()
        );
        copy_files(copy_opts, prev_pdb_path, &self.pdbs_path, true, &self.exclude)?;

        Ok(())
    }

    /// Move PDBs (except excluded) to separate dir, then strip remaining ones
    fn strip_pdbs(&self) -> Result<()> {
        let opts = &self.config.prepare.strip_pdbs;

        info!(
            "Copying/stripping PDBs from \"{}\" to \"{}\"...",
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
            // Simply copy files excluded from stripping
            if opts.exclude.iter().any(|x| relative_path_str.contains(x)) {
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

        info!("Signing files in \"{}\"", self.install_path.display());
        let mut to_sign: Vec<PathBuf> = Vec::new();
        let signable_exts = &self.config.prepare.codesign.sign_exts;

        for file in WalkDir::new(&self.install_path)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| !e.file_type().is_dir())
        {
            let file: DirEntry = file;
            let relative_path = file.path().to_str().unwrap();

            if !signable_exts.iter().any(|x| relative_path.ends_with(x.as_str())) {
                continue;
            }
            to_sign.push(file.path().canonicalize()?)
        }
        sign(to_sign, &self.config.prepare.codesign)?;

        Ok(())
    }

    #[cfg(unix)]
    fn codesign(&self) -> Result<()> {
        warn!("Codesigning is not (yet) supported on this platform.");
        Ok(())
    }

    fn code_analysis(&mut self) -> Result<()> {
        if self.prev_bin_path.is_none() {
            return Ok(());
        }

        // Hash code sections
        info!("Hashing new and old code sections...");
        let prev_build_path = self.prev_bin_path.as_ref().unwrap();
        let in_hashes = get_dir_code_hashes(&self.install_path);
        let old_hashes = get_dir_code_hashes(prev_build_path);

        for (path, file_info) in in_hashes {
            if !old_hashes.contains_key(&path) {
                continue;
            }

            let old_info = old_hashes.get(&path).unwrap();
            if old_info.hash == file_info.hash {
                debug!("File \"{path}\" has identical code hash, can be skipped.");
                // Add filename minus extension to the list so PDBs are also copied from the old
                // version. The trailing "." is included to avoid potential conflicts with files
                // that share the same prefix but.
                let (base, _ext) = path.rsplit_once('.').unwrap();
                self.exclude.insert(format!("{base}."));
            }
        }

        info!("Found {} files to exclude based on code sections.", self.exclude.len());
        Ok(())
    }

    pub fn run(mut self) -> Result<()> {
        if self.find_previous().is_err() {
            warn!("No previous builds found.")
        }

        self.ensure_output_dir()?;
        self.copy()?;
        self.code_analysis()?;
        self.codesign()?;
        self.strip_pdbs()?;
        self.copy_previous()?;

        Ok(())
    }
}

fn copy_files(
    opts: &CopyOptions,
    input: &PathBuf,
    output: &Path,
    copying_old: bool,
    filter: &HashSet<String>,
) -> Result<()> {
    // Non-negotiable excludes
    let mut always_exclude: HashSet<&String> = HashSet::new();
    always_exclude.extend(opts.never_copy.iter());
    // Overrides are also excludes
    opts.overrides.iter().for_each(|(obs_path, _)| {
        always_exclude.insert(obs_path);
    });

    fs::create_dir_all(output)?;

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

        // ToDo figure out if this should be configurable
        if !relative_path.starts_with("bin")
            && !relative_path.starts_with("data")
            && !relative_path.starts_with("obs-plugins")
        {
            continue;
        }

        if always_exclude.iter().any(|f| relative_path_str.contains(*f)) {
            continue;
        }

        let is_binary = BINARY_EXTS.iter().any(|e| relative_path_str.ends_with(e));
        let always_copied = opts.always_copy.iter().any(|e| relative_path_str.contains(e));
        // Include/Exclude filters only apply to binaries except ones that are always copied
        if is_binary && !always_copied {
            // Exclude filtered files when copying new build
            if !copying_old && !filter.is_empty() && filter.iter().any(|f| relative_path_str.starts_with(f)) {
                continue;
            }
            // Include filtered files when copying old build
            if copying_old && !filter.is_empty() && !filter.iter().any(|f| relative_path_str.starts_with(f)) {
                continue;
            }
        } else if copying_old {
            // Do not copy old files for anything that doesn't pass the filters
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
