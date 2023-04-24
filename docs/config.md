# Configuration File

bouf was designed to be very configurable to avoid hardcoding things as much as possible, however,
there are reasonable defaults for things that do not need to be explicitly set.

See the sections below for detailed explanations of all the options.
Also see `extra/config.example.toml` for a complete example configuration file.

Keys are optional unless noted otherwise. 

The bouf configuration file uses the [TOML](https://toml.io/en/) format.

## `[general]` Section

- `branch` (string) - Updater branch to use in path/manifest (default: `stable`)
- `log_level` (string) - Log level to print (default: `info`)

Valid log levels are `trace`, `debug`, `info`, `warn`, and `error`.

## `[env]` Section

*Locations (**required** to be set in the config **or** command line):*
- `input_dir` (path) - directory containing new build
- `output_dir` (path) - directory where data (manifest, ZIPs, installer, updater data) will be written to
- `previous_dir` (path) - directory containing old builds

**Note:** that all of these can be specified via the command line.
Additionally, both `input_dir` and `output_dir` must exist.

*Tool paths (**required** if binaries not in `%PATH%`/`$PATH`):*
- `sevenzip_path` (path) - Path to 7-zip CLI executable
- `makensis_path` (path) - Path to makensis executable
- `pandoc_path` (path) - Path to pandoc executable
- `pdbcopy_path` (path) - Path to pandoc executable

## `[prepare]` Section

- `empty_output_dir` (bool) - Clear the output directory if it is not empty, abort and show an error otherwise (default: `false`)

### `[prepare.copy]` Subsection

*Filters:*
- `never_copy` (array of string) - list of filenames/paths that should never be copied to the output directory (e.g. 32-bit files)
- `always_copy` (array of string) - list of filenames/paths that should always be copied from the input directory (e.g. main OBS exe) (default: `["obs64", "obspython", "obslua", "obs-frontend-api", "obs.dll", "obs.pdb"]`)

*Overrides/External includes:*
- `overrides` (array of [string, string] tuples) - files to be copied to the output from external paths (e.g. game capture)

### `[prepare.codesign]` Subsection

- `skip_sign` (bool) - Skip singing (default: `false`)
- `sign_exts` (array of strings) - file extensions to sign (default: `['exe', 'dll', 'pyd']`)

*signtool parameters (**required** if `skip_sign` is not `true`):*
- `sign_name` (string) - Name of signing certification in certificate store (signtool `/n` parameter)
- `sign_digest` (string) - Hash algorithm to use in signature (signtool `/fd` parameter)
- `sign_ts_serv` (string) - URL of timestamp server to use (signtool `/t` parameter)

## `[prepare.strip_pdbs]` Subsection

- `skip_for_prerelease` (bool) - Skip PDB stripping for pre-release builds (default: `false`)
- `exclude` (array of filenames) - PDB filenames to exclude from stripping

## `[generate]` Section

- `patch_type` (string) - Type of patch to generate, can be `zstd` or `bsdiff_lzma` (default: `zstd`)
- `compress_files` (bool) - Compress non-patch files (default: `true`)

*Filters:*
- `exclude_from_parallel` (array of filenames) - Do not process these files in parallel (e.g. CEF on a RAM-limited machine)
- `exclude_from_removal` (array of filenames) - Do not add these files to the removed files list
- `removed_files` (array of filenames) - Additional files to add to the removed files list

**Note:** bouf will automatically determine a list of deleted files based on which ones appear in older build folders but not the input.
Exclusions are meant for legacy modules that are no longer shipped (e.g. win-mf) so that existing setups continue to work.
Additional deleted files may be specified in cases where the automatic detection will not pick them up,
e.g. when the files are from version that are no longer included in the `previous_dir` folder.

### `[[generate.packages]]` Subsections

**Note:** This is an array of tables (see [TOML Documentation](https://toml.io/en/v1.0.0#array-of-tables)) and can exist multiple times.  
**Note 2:** If omitted, all files are assigned to a package called `core`.  
**Note 3:** The packages are processed in the order specified, files will be added to the first one that matches. 

- `name` (string) - Name of the package (**required**)
- `include_files` (array of strings) - file/path names to include in this package

## `[package]` Section

### `[package.installer]` Subsection

- `skip` (bool) - Whether to skip creating the installer (default: `false`)
- `skip_sign` (bool) - Whether to skip signing the installer (default: `false`)
- `nsis_script` (path) - Path to NSIS script (**required** if `skip` is `false`)

### `[package.updater]` Subsection

- `notes_file` (path) - Path to file containing release notes (RST format) (**required** if not set via command line instead)
- `vc_redist_path` (path) - VC++ redist file which's hash shall be included in the manifest (**required**)
- `pretty_json` (bool) - Whether to pretty-print JSON manifest (default: `false`)

*Signing options:*
- `skip_sign` (bool) - Whether to skip signing the manifest (default: `false`)
- `private_key` (path) - Path to private key file (**required** if not skipped and not set via environment)

**Note:** The private key may instead be specified via a base64 PEM/DER key in the `UPDATER_PRIVATE_KEY` environment variable.

### `[package.zip]` Subsection

- `name` (string) - Name of ZIP file containing the OBS release build (defaults: `OBS-Studio-{version}.zip`)
- `pdb_name` (string) - Name of ZIP file containing unstripped PDBs for this release build (default: `OBS-Studio-{version}-pdbs.zip`)

**Note:** Both support the `{version}` placeholder to be replaced with the OBS version.

- `skip_pdbs_for_prerelease` (bool) - Whether to skip zipping PDBs for pre-release versions (default: `false`)

## `[post]` Section

- `copy_to_old` (bool) - Whether to copy the final directory to `previous_dir` (default: `false`)
