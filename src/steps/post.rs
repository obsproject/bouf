use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use walkdir::{DirEntry, WalkDir};

use crate::utils::misc::get_filename_version;
use crate::Config;

fn copy_directory(input: &PathBuf, output: &PathBuf) -> Result<()> {
    fs::create_dir_all(output)?;
    // Walk dir, honor overrides where necessary
    for file in WalkDir::new(input)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| !e.file_type().is_dir())
    {
        let file: DirEntry = file;
        // Get a path relative to the input directory for lookup/copy path
        let relative_path = file.path().strip_prefix(input)?.to_str().unwrap();
        let file_path = output.join(relative_path);
        // Ensure dir structure exists
        if let Some(_parent) = file_path.parent() {
            fs::create_dir_all(_parent)?;
        }
        fs::copy(file.path(), file_path)?;
    }

    Ok(())
}

pub fn copy_to_old(conf: &Config) -> Result<()> {
    let version = get_filename_version(&conf.obs_version, false);

    let build_out_path = conf.env.previous_dir.join("builds").join(&version);
    let install_path = conf.env.output_dir.join("install");
    copy_directory(&install_path, &build_out_path)?;

    let pdbs_out_path = conf.env.previous_dir.join("pdbs").join(&version);
    let pdbs_path = conf.env.output_dir.join("pdbs");
    copy_directory(&pdbs_path, &pdbs_out_path)?;

    Ok(())
}
