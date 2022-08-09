use bsdiff::patch::patch;
use std::collections::HashMap as StdHashMap;
use std::fs;
use std::fs::File;
use std::hash::Hash;
use std::io::BufReader;
use std::io::Write;
use std::ops::Deref;
use std::path::{Path, PathBuf};

use hashbrown::{HashMap, HashSet};
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressFinish, ProgressStyle};
use rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config::Config;
use crate::utils;
use crate::utils::misc;

#[derive(Serialize, Deserialize, Default)]
pub struct Manifest {
    pub notes: String,
    pub packages: Vec<Package>,
    pub version_major: u8,
    pub version_minor: u8,
    pub version_patch: u8,
    pub beta: u8,
    pub rc: u8,
    // ToDo figure out what to do with this
    pub nightly: u32,
    // Legacy fields
    pub full_installer_x64: String,
    pub full_installer_x86: String,
    pub full_zip_x64: String,
    pub full_zip_x86: String,
    pub small_installer_x64: String,
    pub small_installer_x86: String,
    pub small_zip_x64: String,
    pub small_zip_x86: String,
    pub vc2019_redist_x64: String,
    pub vc2019_redist_x86: String,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Package {
    pub name: String,
    pub removed_files: Vec<String>,
    pub files: Vec<FileEntry>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct FileEntry {
    pub hash: String,
    pub name: String,
    pub size: u64,
}

fn build_hashlist_with_cache(path: &PathBuf) -> HashMap<String, utils::hash::FileInfo> {
    let cache_file = path.join("cache.json");

    let cache: Option<HashMap<String, utils::hash::FileInfo>> = File::open(cache_file.as_path()).ok().and_then(|f| {
        let reader = BufReader::new(f);
        serde_json::from_reader(reader).ok()
    });

    if cache.is_none() {
        println!("[!] No cache found.");
    }

    let hashes = utils::hash::get_dir_hashes(path, cache);

    let file_written = serde_json::to_string_pretty(&hashes).ok().and_then(|j| {
        File::create(cache_file.as_path())
            .ok()
            .and_then(|mut f: File| f.write_all(&j.as_bytes()).ok())
    });

    if file_written.is_none() {
        println!("[!] Cache could not be written")
    }

    hashes
}

pub fn create_patches(conf: &Config) -> Manifest {
    // ToDo clear "out" directory
    // Convert directories to absolute paths
    let new_path = misc::normalize_path(&conf.env.input_dir);
    let old_path = misc::normalize_path(&conf.env.previous_dir);
    let out_path = misc::normalize_path(&conf.env.output_dir);

    // Ensure directories exists
    if !(new_path.exists() && new_path.is_dir()) {
        panic!("Input path does not exist!")
    };
    if !(old_path.exists() && old_path.is_dir()) {
        panic!("Previous versions path does not exist!")
    };
    std::fs::create_dir_all(&out_path).expect("Failed to create output directory");

    println!("[+] Building hash list for new build");
    let new_hashes = utils::hash::get_dir_hashes(&new_path, None);
    println!("[+] Building hash list for old builds");
    let old_hashes = build_hashlist_with_cache(&old_path);
    println!("[+] Determining number of patches to create...");

    // List of all unique patches to generate as old hash => new file
    let mut patch_list: HashMap<String, String> = HashMap::new();
    // Just used for logging
    let mut added_files: HashSet<String> = new_hashes.keys().cloned().collect();
    let mut changed_files: HashSet<String> = HashSet::new();
    let mut removed_files: HashSet<String> = HashSet::new();
    // Used for lookups during generation
    let mut hash_to_file = HashMap::new();

    for (path, fileinfo) in old_hashes {
        let mut rel_path = path[path.find("/").unwrap_or(0) + 1..].to_owned();
        // For backwards-compatibility we remove core and obs-browser package prefixes
        if rel_path.starts_with("core") || rel_path.starts_with("obs-browser") {
            rel_path = rel_path[rel_path.find("/").unwrap_or(0) + 1..].parse().unwrap();
        }

        // If the file was removed, skip it, otherwise remove it from the unique added file list
        if !new_hashes.contains_key(&rel_path) {
            removed_files.insert(rel_path);
            continue;
        } else {
            added_files.remove(&rel_path);
            changed_files.insert(rel_path.clone());
        }

        // Technically this will overwrite existing keys, but that doesn't matter
        hash_to_file.insert(fileinfo.hash.clone(), path.clone());
        patch_list.insert(fileinfo.hash.clone(), rel_path);
    }

    let mut added_files_list = added_files.into_iter().collect::<Vec<_>>();
    added_files_list.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    let mut removed_files_list = removed_files.clone().into_iter().collect::<Vec<_>>();
    removed_files_list.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    let mut changed_files_list = changed_files.into_iter().collect::<Vec<_>>();
    changed_files_list.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

    // Blame rustfmt for this mess.
    println!("    -   Added : {} (see added.txt for details)", added_files_list.len());
    println!(
        "    - Changed : {} (see changed.txt for details)",
        changed_files_list.len()
    );
    println!(
        "    - Removed : {} (see removed.txt for details)",
        removed_files_list.len()
    );
    println!("    - Patches : {}", patch_list.len());

    File::create(out_path.join("added.txt"))
        .unwrap()
        .write_all(added_files_list.join("\n").as_bytes());
    File::create(out_path.join("changed.txt"))
        .unwrap()
        .write_all(changed_files_list.join("\n").as_bytes());
    File::create(out_path.join("removed.txt"))
        .unwrap()
        .write_all(removed_files_list.join("\n").as_bytes());

    // Debug shit
    let test: Map<String, serde_json::Value> = Map::from_iter(
        new_hashes
            .iter()
            .map(|(p, h)| (p.clone(), serde_json::to_value(h).unwrap())),
    );
    let serialized = serde_json::to_string_pretty(&test).unwrap();
    File::create("test.json").unwrap().write_all(&serialized.as_bytes());

    // Map files to packages
    let mut all_files: HashSet<String> = new_hashes.keys().cloned().collect();
    let mut package_map: HashMap<&String, &String> = HashMap::new();
    // This could probably be done better.
    for package in &conf.generate.packages {
        if let Some(_filter) = &package.include_files {
            let _filter_lower: Vec<String> = _filter.iter().map(|x| x.to_lowercase()).collect();
            // This should be a crime.
            all_files
                .iter()
                .filter(|fname| _filter_lower.iter().any(|needle| fname.to_lowercase().contains(needle)))
                .for_each(|fname| {
                    package_map.entry(fname).or_insert(&package.name);
                });

            // We're not messing with removed files while iterating, but only insert if it does not exist yet.
            removed_files
                .iter()
                .filter(|fname| _filter_lower.iter().any(|needle| fname.to_lowercase().contains(needle)))
                .for_each(|fname| {
                    package_map.entry(fname).or_insert(&package.name);
                });
        } else {
            all_files.iter().for_each(|fname| {
                package_map.entry(fname).or_insert(&package.name);
            });

            removed_files.iter().for_each(|fname| {
                package_map.entry(fname).or_insert(&package.name);
            });
        }
    }

    println!("[+] Creating delta-patches...");
    let num = patch_list.len() as u64;
    let style = ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}").unwrap();
    let pbar = ProgressBar::new(num)
        .with_style(style)
        .with_finish(ProgressFinish::AndLeave);
    patch_list
        .par_iter_mut()
        .progress_with(pbar)
        .for_each(|(hash, filename)| {
            let branch = &conf.env.branch;
            let package: &String = package_map.get(filename).unwrap();
            let patch_filename = format!("patches_studio/{branch}/{package}/{filename}/{hash}");
            // Create proper PathBufs from the format we created
            let outfile = out_path.join(patch_filename);
            let oldfile = old_path.join(hash_to_file.get(hash).unwrap());
            let newfile = new_path.join(&filename);
            // Ensure directories exist (Note: this is thread-safe in Rust!)
            fs::create_dir_all(outfile.parent().unwrap());
            utils::bsdiff::create_patch(&oldfile, &newfile, &outfile).expect("Creating hash failed horribly.");
            // "filename" now becomes the hash of the patch file
            // *filename = String::from(outfile.to_str().unwrap());
        });

    // Copy release to non-patch output dir based on packages
    // update_studio

    // Create the manifest
    let mut manifest = Manifest {
        version_major: conf.obs_version.version_major,
        version_minor: conf.obs_version.version_minor,
        version_patch: conf.obs_version.version_patch,
        rc: conf.obs_version.rc,
        beta: conf.obs_version.beta,
        ..Default::default()
    };

    for package in &conf.generate.packages {
        let mut manifest_package = Package {
            name: package.name.to_owned(),
            ..Default::default()
        };
        manifest_package.removed_files = removed_files_list
            .iter()
            .filter(|f| package_map.get(f).unwrap().as_str() == package.name.as_str())
            .cloned()
            .collect();
        manifest_package
            .removed_files
            .sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        manifest_package.files = new_hashes
            .iter()
            .filter(|(f, _)| package_map.get(f).unwrap().as_str() == package.name)
            .map(|(f, v)| FileEntry {
                name: f.to_owned(),
                size: v.size,
                hash: v.hash.to_owned(),
            })
            .collect();
        manifest_package
            .files
            .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        manifest.packages.push(manifest_package);
    }

    manifest
}
