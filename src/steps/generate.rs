use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::io::Write;
use std::path::PathBuf;

use hashbrown::{HashMap, HashSet};
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressFinish, ProgressIterator, ProgressStyle};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use serde_json::Map;

use crate::utils;
use crate::utils::misc;
use crate::utils::config::Config;

#[derive(Serialize, Deserialize, Default)]
pub struct Manifest {
    pub notes: String,
    pub packages: Vec<Package>,
    pub version_major: u8,
    pub version_minor: u8,
    pub version_patch: u8,
    pub beta: u8,
    pub rc: u8,
    // ToDo figure out what to do with this, maybe a timestamp?
    pub nightly: u32,
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
    // Convert directories to absolute paths
    let new_path = misc::normalize_path(&conf.env.input_dir);
    let old_path = misc::normalize_path(&conf.env.previous_dir);
    let out_path = misc::normalize_path(&conf.env.output_dir);

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

    // Add manually specified deleted files as well
    conf.generate.removed_files.iter().for_each(|f| {
        removed_files.insert(f.to_owned());
    });

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

    // Map current/removed files to packages
    let mut all_files: HashSet<String> = new_hashes.keys().cloned().collect();
    all_files.extend(removed_files);

    let mut pattern_package: Vec<(String, &String)> = Vec::new();
    let mut fallback: &String = &String::new();
    let mut package_map: HashMap<&String, &String> = HashMap::new();

    for package in &conf.generate.packages {
        if let Some(_filter) = &package.include_files {
            _filter.iter().for_each(|f| {
                pattern_package.push((f.to_lowercase(), &package.name));
            })
        } else if fallback.is_empty() {
            fallback = &package.name;
        }
    }

    // Iterate over all files, assigning them to packages as needed
    all_files.iter().for_each(|fname| {
        let fname_lower = fname.to_lowercase();

        match pattern_package
            .iter()
            .find(|(pattern, _)| fname_lower.contains(pattern))
        {
            Some((_, branch)) => package_map.insert(fname, branch),
            None => package_map.insert(fname, fallback),
        };
    });

    // Separate out files that are not going to be bsdiff'd in parallel
    let patch_list_st: HashMap<String, String> = patch_list
        .drain_filter(|_, f| conf.generate.exclude_from_parallel.iter().any(|s| f.contains(s)))
        .collect();

    let num = patch_list.len() as u64;
    let style = ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}").unwrap();
    let pbar = ProgressBar::new(num)
        .with_style(style.clone())
        .with_finish(ProgressFinish::AndLeave);

    let branch = &conf.env.branch;
    println!("[+] Creating delta-patches...");
    patch_list.par_iter().progress_with(pbar).for_each(|(hash, filename)| {
        let package: &String = package_map.get(filename).unwrap();
        let patch_filename = format!("updater/patches_studio/{branch}/{package}/{filename}/{hash}");
        // Create proper PathBufs from the format we created
        let outfile = out_path.join(patch_filename);
        let oldfile = old_path.join(hash_to_file.get(hash).unwrap());
        let newfile = new_path.join(&filename);
        // Ensure directories exist (Note: this is thread-safe in Rust!)
        fs::create_dir_all(outfile.parent().unwrap()).expect("Failed creating folder!");
        utils::bsdiff::create_patch(&oldfile, &newfile, &outfile).expect("Creating hash failed horribly.");
    });

    // If any patches were assigned to the non-parallel patch list run them here
    if patch_list_st.len() > 0 {
        let num = patch_list_st.len() as u64;
        let pbar = ProgressBar::new(num)
            .with_style(style.clone())
            .with_finish(ProgressFinish::AndLeave);

        println!("[+] Creating non-parallel delta-patches...");
        patch_list_st.iter().progress_with(pbar).for_each(|(hash, filename)| {
            let package: &String = package_map.get(filename).unwrap();
            let patch_filename = format!("updater/patches_studio/{branch}/{package}/{filename}/{hash}");
            // Create proper PathBufs from the format we created
            let outfile = out_path.join(patch_filename);
            let oldfile = old_path.join(hash_to_file.get(hash).unwrap());
            let newfile = new_path.join(&filename);
            // Ensure directories exist (Note: this is thread-safe in Rust!)
            fs::create_dir_all(outfile.parent().unwrap()).expect("Failed creating folder!");
            utils::bsdiff::create_patch(&oldfile, &newfile, &outfile).expect("Creating hash failed horribly.");
        });
    }

    println!("[+] Copying new build to updater structure...");
    let pbar = ProgressBar::new(new_hashes.len() as u64)
        .with_style(style)
        .with_finish(ProgressFinish::AndLeave);
    new_hashes.par_iter().progress_with(pbar).for_each(|(filename, _)| {
        let branch = &conf.env.branch;
        let package: &String = package_map.get(filename).unwrap();
        let patch_filename = format!("updater/update_studio/{branch}/{package}/{filename}");
        let updater_file = out_path.join(patch_filename);
        let build_file = new_path.join(&filename);
        fs::create_dir_all(updater_file.parent().unwrap()).expect("Failed creating folder!");
        fs::copy(build_file, updater_file).expect("Failed copying file!");
    });

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
            .filter(|f| package_map.get(f).unwrap().as_str() == package.name)
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
