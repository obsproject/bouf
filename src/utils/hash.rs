use std::fmt::Write;
use std::fs::File;
use std::io::{BufReader, Read, Write as IoWrite};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

use blake2::digest::{Update, VariableOutput};
use blake2::Blake2bVar;
use hashbrown::HashMap;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressFinish, ProgressStyle};
use log::{info, warn};
use object::{Object, ObjectSection};
use rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use walkdir::{DirEntry, WalkDir};

const BLAKE2_HASH_SIZE: usize = 20;
const READ_BUFSIZE: usize = usize::pow(2, 16);
const BINARY_EXTS: [&str; 3] = ["exe", "pyd", "dll"];

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct FileInfo {
    pub hash: String,
    pub size: u64,
}

#[cfg(windows)]
fn create_file_info(hash_str: String, file: &File) -> FileInfo {
    let file_meta = file.metadata().expect("Unable to get file metadata");
    FileInfo {
        hash: hash_str,
        size: file_meta.file_size(),
    }
}
#[cfg(unix)]
fn create_file_info(hash_str: String, file: &File) -> FileInfo {
    let file_meta = file.metadata().expect("Unable to get file metadata");
    FileInfo {
        hash: hash_str,
        size: file_meta.size(),
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
        write!(s, "{byte:02x}").unwrap();
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
        let relative_path_str = String::from(relative_path).replace('\\', "/");

        if let Some(_cache_entry) = cache.as_ref().and_then(|_cache| _cache.get(&relative_path_str)) {
            hashes.insert(relative_path_str, _cache_entry.to_owned());
        } else {
            hashes.insert(relative_path_str, FileInfo { ..Default::default() });
        }
    }

    let num = hashes.iter().filter(|(_, v)| v.hash.is_empty()).count() as u64;

    if num == 0 {
        if cache.is_some() {
            info!(" => All file hashes loaded from cache.");
        }
        return hashes;
    }

    info!(" => Hashing {num} files.");
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

/// Create a list of file hashes in a directory, loading existing results from a
/// "cache.json" file inside that directory (if it exists)
/// Error reading/writing a cache file are ignored.
pub fn get_dir_hashes_cache(path: &PathBuf) -> HashMap<String, FileInfo> {
    let cache_file = path.join("cache.json");

    let cache: Option<HashMap<String, FileInfo>> = File::open(cache_file.as_path()).ok().and_then(|f| {
        let reader = BufReader::new(f);
        serde_json::from_reader(reader).ok()
    });

    if cache.is_none() {
        info!("No cache found.");
    }

    let hashes = get_dir_hashes(path, cache);

    let file_written = serde_json::to_string_pretty(&hashes).ok().and_then(|j| {
        File::create(cache_file.as_path())
            .ok()
            .and_then(|mut f: File| f.write_all(j.as_bytes()).ok())
    });

    if file_written.is_none() {
        warn!("Cache could not be written")
    }

    hashes
}

// ToDo make all of this stuff return results
fn hash_file_code(path: &Path) -> FileInfo {
    let mut file = File::open(path).expect("Unable to open file");
    let mut buf = Vec::new();
    let mut hash_buf = [0u8; 20];

    file.read_to_end(&mut buf).unwrap();

    let obj_file = object::File::parse(&*buf).unwrap();
    let mut hasher = Blake2bVar::new(BLAKE2_HASH_SIZE).unwrap();

    obj_file.sections().for_each(|s| {
        hasher.update(s.data().unwrap());
    });

    hasher.finalize_variable(&mut hash_buf).unwrap();

    let mut s = String::with_capacity(2 * BLAKE2_HASH_SIZE);
    for byte in hash_buf {
        write!(s, "{byte:02x}").unwrap();
    }

    create_file_info(s, &file)
}

pub fn get_dir_code_hashes(path: &PathBuf) -> HashMap<String, FileInfo> {
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
        let relative_path_str = String::from(relative_path).replace('\\', "/");

        if !BINARY_EXTS.iter().any(|ext| relative_path_str.ends_with(ext)) {
            continue;
        }

        hashes.insert(relative_path_str, FileInfo { ..Default::default() });
    }

    let num = hashes.iter().filter(|(_, v)| v.hash.is_empty()).count() as u64;

    info!(" => Hashing {num} files' code sections...");
    let style = ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}").unwrap();
    let pbar = ProgressBar::new(num)
        .with_style(style)
        .with_finish(ProgressFinish::AndLeave);
    hashes
        .par_iter_mut()
        .filter(|(_, v)| v.hash.is_empty())
        .progress_with(pbar)
        .for_each(|(f_path, fileinfo)| {
            *fileinfo = hash_file_code(path.join(Path::new(f_path)).as_path());
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
