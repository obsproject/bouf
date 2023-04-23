# Overview

The main binary `bouf` automates the entire process based on the rules laid out in the config file and command line.

Additionally, the following utilities are provided:
- `bouf-sign` - utility for quickly RSA-signing manifest or other files validated by OBS on download
- `bouf-deltas` - stripped down version of bouf only handling generation of delta patches

bouf gets its name - Building OBS Updates Fast(er) - from providing a number of optimisations over the legacy update builder.
It was built to be highly configurable and more easily extendable compared to the previous tooling.

New features compared to legacy tool:
- Parallelisation of file hashing and patch generations
- Zstandard instead of bsdiff
- Support for update branches
- Updated NSIS scripts
- Automatic exclusion of files whose executable contents have not changed
- It's written in Rust, of course :p

## Usage Documentation

- [cli](cli.md) contains documentation on CLI usage
- [config](config.md) contains documentation on the config file format and keys

## General Notes

The generated output from bouf has the following structure:

* `install/` - OBS install files used to build installer/zip files (signed)
* `updater/`
    + `patches_studio/[branch]/[package]/{file}` - delta patches for upload to server
    + `update_studio/[branch]/[package]/{file}` - files split into packages for upload to server
* `pdbs/` - Full PDBs
* `manifest[_<branch>].json` and `manifest[_<branch>].json.sig` for updater
* `added.txt`, `changed.txt`, `unchanged.txt`, and `removed.txt` for manual checks
* `OBS-Studio-<version>-Installer.exe` - NSIS installer (signed)
* `OBS-Studio-<version>.zip` - ZIP file of `install/`
* `OBS-Studio-<version>-pdbs.zip` - Archive of unstripped PDBs
