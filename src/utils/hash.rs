use core::fmt::Write;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
use std::os::linux::fs::MetadataExt;
#[cfg(target_os = "windows")]
use std::os::windows::fs::MetadataExt;

use blake2::digest::{Update, VariableOutput};
use blake2::Blake2bVar;
use hashbrown::HashMap;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressFinish, ProgressStyle};
use rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use walkdir::{DirEntry, WalkDir};

const BLAKE2_HASH_SIZE: usize = 20;
const READ_BUFSIZE: usize = usize::pow(2, 16);

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct FileInfo {
    pub hash: String,
    pub size: u64,
}

#[cfg(target_os = "windows")]
fn create_file_info(hash_str: String, file: &File) -> FileInfo {
    let file_meta = file.metadata().expect("Unable to get file metadata");
    FileInfo {
        hash: hash_str,
        size: file_meta.file_size(),
    }
}
#[cfg(target_os = "linux")]
fn create_file_info(hash_str: String, file: &File) -> FileInfo {
    let file_meta = file.metadata().expect("Unable to get file metadata");
    FileInfo {
        hash: hash_str,
        size: file_meta.st_size(),
    }
}

pub fn hash_file(path: &Path) -> FileInfo {
    let mut file = File::open(path).expect("Unable to open file");
    let mut hasher = Blake2bVar::new(BLAKE2_HASH_SIZE).unwrap();

    let mut read_buf = [0u8; READ_BUFSIZE];
    loop {
        match file.read(&mut read_buf) {
            Ok(read) => {
                if read == 0 {
                    break;
                }
                hasher.update(&read_buf[0..read]);
            }
            Err(err) => panic!("{}", err),
        }
    }

    let mut buf = [0u8; 20];
    hasher.finalize_variable(&mut buf).unwrap();

    let mut s = String::with_capacity(2 * BLAKE2_HASH_SIZE);
    for byte in buf {
        write!(s, "{:02x}", byte).unwrap();
    }

    create_file_info(s, &file)
}

pub fn get_dir_hashes(path: &PathBuf, cache: Option<HashMap<String, FileInfo>>) -> HashMap<String, FileInfo> {
    let mut hashes: HashMap<String, FileInfo> = HashMap::new();

    for file in WalkDir::new(path)
        .min_depth(2)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| !e.file_type().is_dir())
    {
        let file: DirEntry = file;
        // Get a path relative to the input directory
        let relative_path = file.path().strip_prefix(path).unwrap().to_str().unwrap();
        // Internally we always use Unix-style paths, so adjust this here
        let relative_path_str = String::from(relative_path).replace("\\", "/");

        if let Some(_cache_entry) = cache.as_ref().and_then(|_cache| _cache.get(&relative_path_str)) {
            hashes.insert(relative_path_str, _cache_entry.to_owned());
        } else {
            hashes.insert(relative_path_str, FileInfo { ..Default::default() });
        }
    }

    let num = hashes.iter().filter(|(_, v)| v.hash.is_empty()).count() as u64;

    if num == 0 {
        if cache.is_some() {
            println!(" => All file hashes loaded from cache.");
        }
        return hashes;
    }

    println!(" => Hashing {} files.", num);
    let style = ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}").unwrap();
    let pbar = ProgressBar::new(num)
        .with_style(style)
        .with_finish(ProgressFinish::AndLeave);
    hashes
        .par_iter_mut()
        .filter(|(_, v)| v.hash.is_empty())
        .progress_with(pbar)
        .for_each(|(f_path, fileinfo)| {
            *fileinfo = hash_file(path.join(Path::new(f_path)).as_path());
        });

    hashes
}

#[cfg(test)]
mod hash_tests {
    use super::*;

    #[test]
    fn test_blake2() {
        let finfo = hash_file(Path::new("extra/test_files/in.txt"));
        assert_eq!(finfo.hash, "ea08af20e468ff39054c5832b26ee2d80f467045");
    }
}
