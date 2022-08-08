use bsdiff::patch::patch;
use std::collections::HashMap as StdHashMap;
use std::fs;
use std::fs::File;
use std::hash::Hash;
use std::io::BufReader;
use std::io::Write;
use std::ops::Deref;
use std::path::Path;

use hashbrown::{HashMap, HashSet};
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressFinish, ProgressStyle};
use rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};
use serde_json::Map;

use crate::utils;

fn build_hashlist_with_cache(path: &Path) -> HashMap<String, String> {
    let mut _cache: HashMap<String, String>;
    let cache_file = path.join("cache.json");

    let mut cache: Option<&HashMap<String, String>> = None;
    let json: Option<StdHashMap<String, serde_json::Value>> = File::open(cache_file.as_path()).ok().and_then(|f| {
        let reader = BufReader::new(f);
        serde_json::from_reader(reader).ok()
    });

    if let Some(_json) = json {
        _cache = HashMap::new();
        _json.iter().for_each(|(k, v)| {
            _cache.insert(k.clone(), String::from(v.as_str().unwrap()));
        });
        cache = Some(&_cache);
    } else {
        println!("[!] No cache found.");
    }

    let hashes = utils::hash::get_dir_hashes(path, cache);

    let cache_out: Map<String, serde_json::Value> = Map::from_iter(
        hashes
            .iter()
            .map(|(p, h)| (p.clone(), serde_json::to_value(h).unwrap())),
    );

    let serialised = serde_json::to_string_pretty(&cache_out).unwrap();
    if let Ok(mut f) = File::create(cache_file.as_path()) {
        f.write_all(&serialised.as_bytes());
    }

    hashes
}

pub fn create_patches(new_path: &Path, old_path: &Path, out_path: &Path) {
    // Ensure directories exists
    // ToDo clear "out" directory
    std::fs::create_dir_all(new_path).expect("Failed to create input directory");
    std::fs::create_dir_all(old_path).expect("Failed to create old versions directory");
    std::fs::create_dir_all(out_path).expect("Failed to create output directory");

    println!("[+] Building hash list for new build");
    let new_hashes = utils::hash::get_dir_hashes(new_path, None);
    println!("[+] Building hash list for old builds");
    let old_hashes = build_hashlist_with_cache(old_path);
    println!("[+] Determining number of patches to create...");

    // List of all unique patches to generate as old hash => new file
    let mut patch_list: HashMap<String, String> = HashMap::new();
    // Just used for logging
    let mut added_files: HashSet<String> = new_hashes.keys().cloned().collect();
    let mut changed_files: HashSet<String> = HashSet::new();
    let mut removed_files: HashSet<String> = HashSet::new();
    // Used for lookups during generation
    let mut hash_to_file = HashMap::new();

    for (path, hash) in old_hashes {
        let rel_path = path[path.find("/").unwrap_or(0) + 1..].to_owned();

        // If the file was removed, skip it, otherwise remove it from the unique added file list
        if !new_hashes.contains_key(&rel_path) {
            removed_files.insert(rel_path);
            continue;
        } else {
            added_files.remove(&rel_path);
            changed_files.insert(rel_path.clone());
        }

        // Technically this will overwrite existing keys, but that doesn't matter
        hash_to_file.insert(hash.clone(), path.clone());
        patch_list.insert(hash.clone(), rel_path);
    }

    let mut added_files_list = added_files.into_iter().collect::<Vec<_>>();
    added_files_list.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    let mut removed_files_list = removed_files.into_iter().collect::<Vec<_>>();
    removed_files_list.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    let mut changed_files_list = changed_files.into_iter().collect::<Vec<_>>();
    changed_files_list.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

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

    // ToDo split shit up into "packages"
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
            let patch_filename = filename.to_owned() + "/" + hash;
            let outfile = out_path.join(patch_filename);
            let oldfile = old_path.join(hash_to_file.get(hash).unwrap());
            let newfile = new_path.join(&filename);
            // Ensure directories exist (Note: this is thread-safe in Rust!)
            fs::create_dir_all(outfile.parent().unwrap());
            utils::bsdiff::create_patch(&oldfile, &newfile, &outfile).expect("Creating hash failed horribly.");
            // "filename" now becomes the hash of the patch file
            *filename = String::from(outfile.to_str().unwrap());
        });
}
