# BOUF - Building OBS Updates Fast(er)

`bouf` is the next-generation utility for preparing and building OBS Studio Windows release binaries and updater delta patches.

The main binary `bouf` automates the entire process based on the rules laid out in the config file and command line.

The generated output has the following structure:

* `install/` - OBS install files used to build installer/zip files (signed)
* `updater/`
  + `patches_studio/[branch]/{core,obs-browser}` - delta patches for upload to server 
  + `update_studio/[branch]/{core,obs-browser}` - files split into packages for upload to server
* `pdbs/` - Full PDBs
* `manifest[_<branch>].json` and `manifest[_<branch>].json.sig` for updater
* `added.txt`, `changed.txt`, `unchanged.txt`, and `removed.txt` for manual checks 
* `OBS-Studio-<version>-Installer.exe` - NSIS installer (signed)
* `OBS-Studio-<version>.zip` - ZIP file of `install/`
* `OBS-Studio-<version>-pdbs.zip` - Archive of unstripped PDBs

Additionally, the following utilities are provided:
* `bouf-sign` to sign files verified by the OBS updater (manifest, updater.exe, whatsnew, branches, etc.)
* `bouf-deltas` to create delta patches and nothing else

## Usage:

```
bouf 0.3.2

USAGE:
    bouf.exe [OPTIONS] --config <Config file> --version <Major.Minor.Patch[-(rc|beta)Num]>

OPTIONS:
        --beta <Beta number>                            Beta number
        --branch <Beta branch>                          Branch used in manifest name/update files
    -c, --config <Config file>
        --clear-output                                  Clear existing output directory
        --commit <commit hash>                          Commit hash used in manifest
        --exclude <FILTER>
    -h, --help                                          Print help information
    -i, --input <new build>
        --include <FILTER>
        --note-file <file.rtf>                          File containing release notes
    -o, --output <output dir>
    -p, --previous <old builds>
        --private-key <file.pem>                        Falls back to "UPDATER_PRIVATE_KEY" env var
        --rc <RC number>                                RC number
        --skip-codesigning                              Skip codesigning
        --skip-installer                                Skip creating NSIS installer
        --skip-manifest-signing                         Skip signing manifest
        --skip-patches                                  Skip creating delta patches
        --updater-data-only                             Create only delta patches and manifest
    -v, --version <Major.Minor.Patch[-(rc|beta)Num]>    OBS main version
```


**Note:** A valid configuration file based on `extra/config.example.toml` is required (see `extra/ci` for an example).

Some parameters can be set via environment variables (e.g. secrets):
- `UPDATER_PRIVATE_KEY` - updater signing key (PEM, encoded as base64)

## License

The source code found in `src/` is licensed under Apache-2 (see `LICENSE.txt`).

Files in `extra/nsis` may have other licenses and exist primarily for CI usage and testing,
and may not be redistributed under the Apache-2 terms.

# ToDo

- Go through older code and replace `.expect()`s and `panic!`s with anyhow errors 
  + This will require some larger changes in some codepaths, do this later...
- Use proper logging with levels and timestamped output
- Figure out how to deal with nightlies
  + Disable copy to previous directory?
  + No deltas to avoid problems?
