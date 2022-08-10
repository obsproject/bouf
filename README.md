# BOUF - Building OBS Updates Fast(er)

`bouf` is the next-generation utility for preparing and building OBS Studio Windows release binaries and updater delta patches.

The main binary `bouf` automates the entire process based on the rules laid out in the config file and command line.

Additionally, various steps and utilities are provided as separate binaries:

* `bouf-prep` - Prepares install directory, handles codesigning and PDB stripping
* `bouf-buildpatches` - Creates patch files and manifest for use with OBS updater
* `bouf-pack` - Packages the prepared install into ZIP and NSIS installer, and finalises/signs the manifest
* `bouf-sign` - Standalone utility to sign files verified by the OBS updater (manifest, updater.exe, whatsnew, branches, etc.)

The generated output has the following structure:

* `install/` - OBS install files used to build installer/zip files (signed)
* `updater/`
  + `patches_studio/<branch>/{core,obs-browser}` - delta patches for upload to server 
  + `update_studio/<branch>/{core,obs-browser}` - files split into packages for upload to server
* `pdbs/` - Full PDBs
* `manifest_<branch>.json` and `manifest_<branch>.json.sig` for updater
* `added.txt`, `changed.txt`, and `removed.txt` for manual checks 
* `OBS-Studio-<version>-Installer.exe` - NSIS installer (signed)
* `OBS-Studio-<version>.zip` - ZIP file of `install/`
* `OBS-Studio-<version>-pdbs.zip` - Archive of unstripped PDBs

## Usage:

`bouf.exe --config <config.toml> --version <verrsion> --input C:/obs/build/output [--beta <num> / --rc <num>] [--skip-patches] [--skip-installer] [--branch <stable/nightly/beta>] [--notes <path/to/notes.rtf>]`

**Note:** A valid configuration file based on `config.example.toml` is required.

Some parameters can be set via environment variables (e.g. secrets):
- `UPDATER_PRIVATE_KEY` - updater signing key (PEM or DER, encoded as base64)

## ToDo

- Packaging
- Signing manifest/updater
- Figure out a license
- 