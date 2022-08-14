# BOUF - Building OBS Updates Fast(er)

`bouf` is the next-generation utility for preparing and building OBS Studio Windows release binaries and updater delta patches.

The main binary `bouf` automates the entire process based on the rules laid out in the config file and command line.

Additionally, the `bouf-sign` utility is provided to sign files verified by the OBS updater (manifest, updater.exe, whatsnew, branches, etc.)

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

`bouf.exe --config <config.toml> --version <version> [--beta <num> / --rc <num>] --new C:/obs/repo/build/output --old C:/obs/releases/old --out C:/obs/releases/new [--skip-patches] [--skip-installer] [--branch <stable/nightly/beta>] [--notes-file <path/to/notes.rtf>] [--private-key <path/to/privkey.pem>]`

May not be up to date, use `bouf.exe -h` to see full help.

**Note:** A valid configuration file based on `config.example.toml` is required (see `extra/ci` for an example).

Some parameters can be set via environment variables (e.g. secrets):
- `UPDATER_PRIVATE_KEY` - updater signing key (PEM, encoded as base64)

## ToDo

- Figure out a license
- Cleanup, bugfixes, rewrites...
  + See "ToDo"s in source code and `rustc` warnings (it angry)
- Also probably more tests.
