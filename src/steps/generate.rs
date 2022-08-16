use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::io::Write;
use std::path::PathBuf;

use hashbrown::{HashMap, HashSet};
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressFinish, ProgressIterator, ProgressStyle};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::utils;
use crate::utils::config::Config;
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
    pub commit: String,
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

struct Patch {
    hash: String,
    name: String,
    old_file: PathBuf,
    new_file: PathBuf,
}

/// Create a list of file hashes in a directory, loading existing results from a
/// "cache.json" file inside that directory (if it exists)
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

/// Write text file, logging but ultimately ignoring errors
fn write_file_unchecked(filename: PathBuf, contents: String) {
    if let Ok(mut f) = File::create(&filename) {
        if let Err(e) = f.write_all(contents.as_bytes()) {
            println!("Writing {} failed: {}", filename.display(), e);
        }
    }
}

pub fn create_manifest_and_patches(conf: &Config, skip_patches: bool, skipped_prep: bool) -> Manifest {
    // Convert directories to absolute paths
    let new_path: PathBuf;
    let old_path = misc::normalize_path(&conf.env.previous_dir);
    let out_path = misc::normalize_path(&conf.env.output_dir);

    if skipped_prep {
        new_path = misc::normalize_path(&conf.env.input_dir);
    } else {
        new_path = misc::normalize_path(&conf.env.output_dir.join("install"));
    }

    std::fs::create_dir_all(&out_path).expect("Failed to create output directory");

    println!("[+] Building hash list for new build");
    let new_hashes = utils::hash::get_dir_hashes(&new_path, None);
    println!("[+] Building hash list for old builds");
    let old_hashes = build_hashlist_with_cache(&old_path);
    println!("[+] Determining number of patches to create...");

    // List of all unique patches to generate as old hash => new file
    let mut patch_list: Vec<Patch> = Vec::new();
    // Sets of added/new files as well as removed/seen ones for processing
    let mut added_files: HashSet<String> = new_hashes.keys().cloned().collect();
    let new_files: HashSet<String> = added_files.clone();
    let mut removed_files: HashSet<String> = HashSet::new();
    let mut seen_hashes: HashSet<(String, String)> = HashSet::new();
    // Just used for logging
    let mut changed_files: HashSet<String> = HashSet::new();

    for (path, fileinfo) in old_hashes {
        // Strip version (first folder name) from path
        let mut rel_path = path[path.find("/").unwrap_or(0) + 1..].to_owned();
        // For backwards-compatibility: Remove "core/" and "obs-browser/" package prefixes in filenames
        if rel_path.starts_with("core") || rel_path.starts_with("obs-browser") {
            rel_path = rel_path[rel_path.find("/").unwrap_or(0) + 1..].parse().unwrap();
        }
        let seen_key = (fileinfo.hash.to_owned(), rel_path.to_owned());
        // Skip (hash, filename) pairs we already added to the patch list
        if seen_hashes.contains(&seen_key) {
            continue;
        } else if !new_hashes.contains_key(&rel_path) {
            // Only add files to removed that do not match any exclusion filter
            if !conf.generate.exclude_from_removal.iter().any(|s| rel_path.contains(s)) {
                removed_files.insert(rel_path);
            }
            continue;
        } else {
            added_files.remove(&rel_path);
            changed_files.insert(rel_path.clone());
        }

        if !skip_patches {
            patch_list.push(Patch {
                hash: fileinfo.hash.clone(),
                name: rel_path.clone(),
                old_file: old_path.join(path),
                new_file: new_path.join(rel_path),
            });
        }

        seen_hashes.insert(seen_key);
    }

    // Add removed files from config as well to allow deleting additional files
    // which may no longer be present in versions in the "old" directory.
    removed_files.extend(conf.generate.removed_files.iter().cloned());

    // Convert to Vec for sorting/saving to disk
    let mut added_files_list = added_files.into_iter().collect::<Vec<_>>();
    let mut removed_files_list = removed_files.clone().into_iter().collect::<Vec<_>>();
    let mut changed_files_list = changed_files.into_iter().collect::<Vec<_>>();
    added_files_list.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    removed_files_list.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    changed_files_list.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    write_file_unchecked(out_path.join("added.txt"), added_files_list.join("\n"));
    write_file_unchecked(out_path.join("changed.txt"), changed_files_list.join("\n"));
    write_file_unchecked(out_path.join("removed.txt"), removed_files_list.join("\n"));
    println!("    -   Added : {} (see added.txt)", added_files_list.len());
    println!("    - Changed : {} (see changed.txt)", changed_files_list.len());
    println!("    - Removed : {} (see removed.txt)", removed_files_list.len());
    println!("    - Patches : {}", patch_list.len());

    // This is a simple list we use to sort files into packages containing (pattern, package name) tuples
    let mut pattern_list: Vec<(&String, &String)> = Vec::new();
    // If a file matches no pattern, we use the first package without rules as the fallback.
    // The config validator ensures this exists, but we have to initialise the value to something,
    // so use the last entry.
    let mut default_pkg: &String = &conf.generate.packages.last().unwrap().name;
    for package in &conf.generate.packages {
        match &package.include_files {
            Some(patterns) => {
                for pattern in patterns {
                    pattern_list.push((pattern, &package.name));
                }
            }
            None => {
                default_pkg = &package.name;
                break;
            }
        }
    }

    // Map current/removed files to packages in filename -> package name map.
    // (we will look up the same filename multiple times, so precomputing this is probably more efficient!)
    let mut package_map: HashMap<&String, &String> = HashMap::new();
    for filename in new_files.union(&removed_files) {
        if let Some((_, pkg_name)) = pattern_list.iter().find(|(pattern, _)| filename.contains(&**pattern)) {
            package_map.insert(&filename, &pkg_name);
        }
    }

    // Create the manifest
    let mut manifest = Manifest {
        version_major: conf.obs_version.version_major,
        version_minor: conf.obs_version.version_minor,
        version_patch: conf.obs_version.version_patch,
        rc: conf.obs_version.rc,
        beta: conf.obs_version.beta,
        commit: conf.obs_version.commit.to_owned(),
        ..Default::default()
    };

    for package in &conf.generate.packages {
        let mut manifest_package = Package {
            name: package.name.to_owned(),
            ..Default::default()
        };

        manifest_package.removed_files = removed_files_list
            .iter()
            .filter(|f| **package_map.get(f).unwrap_or(&default_pkg) == package.name)
            .cloned()
            .collect();
        manifest_package.files = new_hashes
            .iter()
            .filter(|(f, _)| **package_map.get(f).unwrap_or(&default_pkg) == package.name)
            .map(|(f, v)| FileEntry {
                name: f.to_owned(),
                size: v.size,
                hash: v.hash.to_owned(),
            })
            .collect();

        // Sort file lists alphabetically for a nicer manifest
        manifest_package
            .removed_files
            .sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        manifest_package
            .files
            .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        manifest.packages.push(manifest_package);
    }

    // Sort packages by name as well
    manifest
        .packages
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    // we will re-use these for all following loops
    let branch = &conf.env.branch;
    let style = ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}").unwrap();

    let pbar = ProgressBar::new(new_hashes.len() as u64)
        .with_style(style.clone())
        .with_finish(ProgressFinish::AndLeave);

    println!("[+] Copying new build to updater structure...");
    new_hashes.par_iter().progress_with(pbar).for_each(|(filename, _)| {
        let package: &String = package_map.get(filename).unwrap_or(&default_pkg);
        let patch_filename = format!("updater/update_studio/{branch}/{package}/{filename}");
        let updater_file = out_path.join(patch_filename);
        let build_file = new_path.join(&filename);
        fs::create_dir_all(updater_file.parent().unwrap()).expect("Failed creating folder!");
        fs::copy(build_file, updater_file).expect("Failed copying file!");
    });

    if skip_patches || patch_list.is_empty() {
        println!("[*] No patches to create or patch generation skipped");
        return manifest;
    }

    // Patches to generate in single-threaded mode (e.g. CEF on CI)
    let patch_list_st: Vec<&Patch> = patch_list
        .iter()
        .filter(|p| conf.generate.exclude_from_parallel.iter().any(|s| p.name.contains(s)))
        .collect();
    // Patches to generate in multi-threaded mode (yay rayon)
    let patch_list_mt: Vec<&Patch> = patch_list
        .iter()
        .filter(|p| !conf.generate.exclude_from_parallel.iter().any(|s| p.name.contains(s)))
        .collect();

    println!("[+] Creating delta-patches...");
    let num = patch_list_mt.len() as u64;
    let pbar = ProgressBar::new(num)
        .with_style(style.clone())
        .with_finish(ProgressFinish::AndLeave);
    patch_list_mt.par_iter().progress_with(pbar).for_each(|patch| {
        let package: &String = package_map.get(&patch.name).unwrap_or(&default_pkg);
        let patch_filename = format!(
            "updater/patches_studio/{}/{}/{}/{}",
            branch, package, patch.name, patch.hash
        );
        let outfile = out_path.join(patch_filename);
        // Ensure directories exist (Note: this is thread-safe in Rust!)
        fs::create_dir_all(outfile.parent().unwrap()).expect("Failed creating folder!");
        utils::bsdiff::create_patch(&patch.old_file, &patch.new_file, &outfile)
            .expect("Creating hash failed horribly.");
    });

    // If any patches were assigned to the non-parallel patch list run them here
    if patch_list_st.len() > 0 {
        let num = patch_list_st.len() as u64;
        let pbar = ProgressBar::new(num)
            .with_style(style.clone())
            .with_finish(ProgressFinish::AndLeave);

        println!("[+] Creating non-parallel delta-patches...");
        patch_list_st.iter().progress_with(pbar).for_each(|patch| {
            let package: &String = package_map.get(&patch.name).unwrap_or(&default_pkg);
            let patch_filename = format!(
                "updater/patches_studio/{}/{}/{}/{}",
                branch, package, patch.name, patch.hash
            );
            let outfile = out_path.join(patch_filename);
            fs::create_dir_all(outfile.parent().unwrap()).expect("Failed creating folder!");
            utils::bsdiff::create_patch(&patch.old_file, &patch.new_file, &outfile)
                .expect("Creating hash failed horribly.");
        });
    }

    manifest
}
