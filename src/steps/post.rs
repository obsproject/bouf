use std::{fs, io};

use walkdir::{DirEntry, WalkDir};

use crate::utils::misc::get_filename_version;
use crate::Config;

pub fn copy_to_old(conf: &Config) -> io::Result<()> {
    let version = get_filename_version(&conf.obs_version, false);

    let out_path = conf.env.previous_dir.join(version);
    let inp_path = conf.env.output_dir.join("install");

    std::fs::create_dir_all(&out_path)?;
    // Walk dir, honor overrides where necessary
    for file in WalkDir::new(&inp_path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| !e.file_type().is_dir())
    {
        let file: DirEntry = file;
        // Get a path relative to the input directory for lookup/copy path
        let relative_path = file.path().strip_prefix(&inp_path).unwrap().to_str().unwrap();
        let file_path = out_path.join(relative_path);
        // Ensure dir structure exists
        if let Some(_parent) = file_path.parent() {
            fs::create_dir_all(_parent)?;
        }
        fs::copy(file.path(), file_path)?;
    }

    Ok(())
}
