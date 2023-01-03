use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use hashbrown::{HashMap, HashSet};
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressFinish, ProgressIterator, ProgressStyle};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::models::config::Config;
use crate::models::manifest::{FileEntry, Manifest, Package};
use crate::utils;
use crate::utils::hash::FileInfo;
use crate::utils::misc;

struct Patch {
    hash: String,
    name: String,
    old_file: PathBuf,
    new_file: PathBuf,
}

pub struct Generator<'a> {
    config: &'a Config,

    inp_path: PathBuf,
    old_path: PathBuf,
    out_path: PathBuf,

    analysis: Option<Analysis>,
}

#[derive(Default)]
struct Analysis {
    // List of all unique patches to generate as old hash => new file
    patch_list: Vec<Patch>,
    // Input file hashmap
    input_map: HashMap<String, FileInfo>,
    // Sets of added/new files as well as removed/seen ones for processing
    added_files: HashSet<String>,
    all_files: HashSet<String>,
    changed_files: HashSet<String>,
    removed_files: HashSet<String>,
    unchanged_files: HashSet<String>,
    // Map of removed/input file names to package
    default_pkg: String,
    package_map: HashMap<String, String>,
}

impl<'a> Generator<'a> {
    pub fn init(conf: &'a Config, ran_prep: bool) -> Self {
        let mut ret = Self {
            config: conf,
            old_path: misc::normalize_path(&conf.env.previous_dir),
            out_path: misc::normalize_path(&conf.env.output_dir),
            inp_path: misc::normalize_path(&conf.env.input_dir),
            analysis: None,
        };
        if ran_prep {
            ret.inp_path = misc::normalize_path(&conf.env.output_dir.join("install"));
        }
        ret
    }

    /// Create mapping of filenames to pattern
    fn fill_package_map(&mut self) {
        let analysis = self.analysis.as_mut().unwrap();
        // This is a simple list we use to sort files into packages
        // containing (pattern, package_name) tuples
        let mut pattern_list: Vec<(&String, &String)> = Vec::new();
        // If a file matches no pattern, we use the first package without rules as the fallback.
        // The config validator ensures this exists, we just initialise with the last entry.
        analysis.default_pkg = self.config.generate.packages.last().unwrap().name.to_owned();
        for package in &self.config.generate.packages {
            match &package.include_files {
                Some(patterns) => {
                    for pattern in patterns {
                        pattern_list.push((pattern, &package.name));
                    }
                }
                None => {
                    analysis.default_pkg = package.name.to_owned();
                    break;
                }
            }
        }

        // (we will look up the same filename multiple times, so precomputing this is probably more efficient!)
        for filename in analysis.all_files.iter() {
            if let Some((_, pkg_name)) = pattern_list.iter().find(|(pattern, _)| filename.contains(*pattern)) {
                analysis.package_map.insert(filename.to_owned(), (*pkg_name).to_owned());
            }
        }
    }

    fn write_file_lists(&mut self, analysis: &Analysis) {
        // Convert to Vec for sorting/saving to disk
        let added_files_list = get_sorted_list(&analysis.added_files);
        let removed_files_list = get_sorted_list(&analysis.removed_files);
        let changed_files_list = get_sorted_list(&analysis.changed_files);
        let unchanged_files_list = get_sorted_list(&analysis.unchanged_files);

        println!("  -     Added : {} (see added.txt)", added_files_list.len());
        println!("  -   Changed : {} (see changed.txt)", changed_files_list.len());
        println!("  - Unchanged : {} (see unchanged.txt)", unchanged_files_list.len());
        println!("  -   Removed : {} (see removed.txt)", removed_files_list.len());
        println!("  -   Patches : {}", analysis.patch_list.len());

        write_file_unchecked(self.out_path.join("added.txt"), added_files_list.join("\n"));
        write_file_unchecked(self.out_path.join("removed.txt"), removed_files_list.join("\n"));
        write_file_unchecked(self.out_path.join("changed.txt"), changed_files_list.join("\n"));
        write_file_unchecked(self.out_path.join("unchanged.txt"), unchanged_files_list.join("\n"));
    }

    /// Run analysis steps (hashing, creating list of patches, etc.)
    fn analyse(&mut self, skip_patches: bool) {
        let mut analysis = Analysis { ..Default::default() };

        println!("[+] Building hash list for new build");
        analysis.input_map = utils::hash::get_dir_hashes(&self.inp_path, None);
        println!("[+] Building hash list for old builds");
        let old_hashes = utils::hash::get_dir_hashes_cache(&self.old_path);
        println!("[+] Building list of changes/patches...");

        // Initialise added files with all new files, and remove duplicates later
        analysis.all_files = analysis.input_map.keys().cloned().collect();
        analysis.added_files = analysis.all_files.clone();
        // List of "seen" (hash, path) pairs to skip over duplicates
        let mut seen_hashes: HashSet<(String, String)> = HashSet::new();

        for (path, fileinfo) in old_hashes {
            // Strip version (first folder name) from path
            let mut rel_path = path[path.find('/').unwrap_or(0) + 1..].to_owned();
            // For backwards-compatibility: Remove "core/" and "obs-browser/" package prefixes in filenames
            if rel_path.starts_with("core") || rel_path.starts_with("obs-browser") {
                rel_path = rel_path[rel_path.find('/').unwrap_or(0) + 1..].parse().unwrap();
            }

            // Skip (hash, filename) pairs we already added to the patch list
            let seen_key = (fileinfo.hash.to_owned(), rel_path.to_owned());
            if seen_hashes.contains(&seen_key) {
                continue;
            } else if !analysis.input_map.contains_key(&rel_path) {
                // Only add files to removed that do not match any exclusion filter
                if !self
                    .config
                    .generate
                    .exclude_from_removal
                    .iter()
                    .any(|s| rel_path.contains(s))
                {
                    analysis.removed_files.insert(rel_path);
                }
                continue;
            } else {
                analysis.added_files.remove(&rel_path);
                // Skip if old and new hash match
                if analysis.input_map.get(&rel_path).unwrap().hash == fileinfo.hash {
                    analysis.unchanged_files.insert(rel_path.clone());
                    continue;
                }
                analysis.changed_files.insert(rel_path.clone());
            }

            if !skip_patches {
                analysis.patch_list.push(Patch {
                    hash: fileinfo.hash.clone(),
                    name: rel_path.clone(),
                    old_file: self.old_path.join(path),
                    new_file: self.inp_path.join(rel_path),
                });
            }

            seen_hashes.insert(seen_key);
        }

        // Add removed files from config as well to allow deleting additional files
        // which may no longer be present in versions in the "old" directory.
        analysis
            .removed_files
            .extend(self.config.generate.removed_files.iter().cloned());
        analysis.all_files.extend(analysis.removed_files.iter().cloned());

        self.write_file_lists(&analysis);

        self.analysis = Some(analysis);
    }

    /// Create updater manifest from analysis results
    fn create_manifest(&self) -> Manifest {
        let analysis = self.analysis.as_ref().unwrap();
        let mut manifest = Manifest::new().with_version(&self.config.obs_version);

        for package in &self.config.generate.packages {
            let mut manifest_package = Package {
                name: package.name.to_owned(),
                ..Default::default()
            };

            manifest_package.removed_files = analysis
                .removed_files
                .iter()
                .filter(|&f| *analysis.package_map.get(f).unwrap_or(&analysis.default_pkg) == package.name)
                .cloned()
                .collect();
            manifest_package.files = analysis
                .input_map
                .iter()
                .filter(|(f, _)| *analysis.package_map.get(&**f).unwrap_or(&analysis.default_pkg) == package.name)
                .map(|(f, v)| FileEntry {
                    name: f.to_owned(),
                    size: v.size,
                    hash: v.hash.to_owned(),
                })
                .collect();

            // Sort file lists alphabetically for a nicer manifest
            manifest_package.removed_files.sort_by_key(|a| a.to_lowercase());
            manifest_package.files.sort_by_key(|a| a.name.to_lowercase());

            manifest.packages.push(manifest_package);
        }

        // Sort packages by name as well
        manifest.packages.sort_by_key(|a| a.name.to_lowercase());

        manifest
    }

    /// Copy build to updater directory structure
    fn copy_build(&self) {
        let analysis = self.analysis.as_ref().unwrap();
        std::fs::create_dir_all(&self.out_path).expect("Failed to create output directory");

        let style =
            ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}").unwrap();
        let progress_bar = ProgressBar::new(analysis.input_map.len() as u64)
            .with_style(style)
            .with_finish(ProgressFinish::AndLeave);

        let branch = &self.config.env.branch;
        println!("[+] Copying new build to updater structure...");
        analysis
            .input_map
            .par_iter()
            .progress_with(progress_bar)
            .for_each(|(filename, _)| {
                let package: &String = analysis.package_map.get(filename).unwrap_or(&analysis.default_pkg);
                let patch_filename = format!("updater/update_studio/{branch}/{package}/{filename}");
                let updater_file = self.out_path.join(patch_filename);
                let build_file = self.inp_path.join(filename);
                fs::create_dir_all(updater_file.parent().unwrap()).expect("Failed creating folder!");
                fs::copy(build_file, updater_file).expect("Failed copying file!");
            });
    }

    /// Create patches for old -> new folder
    /// (Note: can be called standalone to just create deltas)
    pub fn create_patches(&mut self) -> Result<()> {
        if self.analysis.is_none() {
            self.analyse(false);
            self.fill_package_map();
        }
        std::fs::create_dir_all(&self.out_path).expect("Failed to create output directory");
        let analysis = self.analysis.as_ref().unwrap();

        // Patches to generate in single-threaded mode (e.g. CEF on CI)
        let patch_list_st: Vec<&Patch> = analysis
            .patch_list
            .iter()
            .filter(|p| {
                self.config
                    .generate
                    .exclude_from_parallel
                    .iter()
                    .any(|s| p.name.contains(s))
            })
            .collect();
        // Patches to generate in multi-threaded mode (yay rayon)
        let patch_list_mt: Vec<&Patch> = analysis
            .patch_list
            .iter()
            .filter(|p| {
                !self
                    .config
                    .generate
                    .exclude_from_parallel
                    .iter()
                    .any(|s| p.name.contains(s))
            })
            .collect();

        let branch = &self.config.env.branch;
        let num = patch_list_mt.len() as u64;

        let style =
            ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}").unwrap();
        let progress_bar_mt = ProgressBar::new(num)
            .with_style(style.clone())
            .with_finish(ProgressFinish::AndLeave);

        println!("[+] Creating delta-patches...");
        patch_list_mt
            .par_iter()
            .progress_with(progress_bar_mt)
            .for_each(|patch| {
                let package: &String = analysis.package_map.get(&patch.name).unwrap_or(&analysis.default_pkg);
                let patch_filename = format!(
                    "updater/patches_studio/{}/{}/{}/{}",
                    branch, package, patch.name, patch.hash
                );
                let outfile = self.out_path.join(patch_filename);
                // Ensure directories exist (Note: this is thread-safe in Rust!)
                fs::create_dir_all(outfile.parent().unwrap()).expect("Failed creating folder!");
                utils::bsdiff::create_patch(&patch.old_file, &patch.new_file, &outfile)
                    .expect("Creating delta patch failed horribly.");
            });

        // If any patches were assigned to the non-parallel patch list run them here
        if !patch_list_st.is_empty() {
            let num = patch_list_st.len() as u64;
            let progress_bar_st = ProgressBar::new(num)
                .with_style(style)
                .with_finish(ProgressFinish::AndLeave);

            println!("[+] Creating non-parallel delta-patches...");
            patch_list_st.iter().progress_with(progress_bar_st).for_each(|patch| {
                let package: &String = analysis.package_map.get(&patch.name).unwrap_or(&analysis.default_pkg);
                let patch_filename = format!(
                    "updater/patches_studio/{}/{}/{}/{}",
                    branch, package, patch.name, patch.hash
                );
                let outfile = self.out_path.join(patch_filename);
                fs::create_dir_all(outfile.parent().unwrap()).expect("Failed creating folder!");
                utils::bsdiff::create_patch(&patch.old_file, &patch.new_file, &outfile)
                    .expect("Creating delta patch failed horribly.");
            });
        }

        Ok(())
    }

    pub fn run(mut self, skip_patches: bool) -> Result<Manifest> {
        // ToDo add errors to individual steps
        self.analyse(skip_patches);
        self.fill_package_map();
        self.copy_build();
        let manifest = self.create_manifest();

        let analysis = self.analysis.as_ref().unwrap();
        if skip_patches || analysis.patch_list.is_empty() {
            println!("[*] No patches to create or patch generation skipped");
            return Ok(manifest);
        }

        self.create_patches()?;

        Ok(manifest)
    }
}

/// Write text file, logging but ultimately ignoring errors
fn write_file_unchecked(filename: PathBuf, contents: String) {
    if let Ok(mut f) = fs::File::create(&filename) {
        if let Err(e) = f.write_all(contents.as_bytes()) {
            println!("Writing {} failed: {}", filename.display(), e);
        }
    }
}

/// Turn string hashset into sorted vector
fn get_sorted_list(inp: &HashSet<String>) -> Vec<String> {
    let mut list = inp.into_iter().cloned().collect::<Vec<_>>();
    list.sort_by_key(|a| a.to_lowercase());

    list
}
