# Command Line Interface (CLI)

The command line interface allows a number of options to by dynamically set without having to modify the config file each time.
This includes things that are expected to change each time such as the version, as well as a few other things to make
testing more bearable (e.g. skipping installer creation).

**Note:** A valid configuration fileis required (see [config](config.md) and `extra/config.example.toml`).

Some parameters can be set via environment variables (e.g. secrets):
- `UPDATER_PRIVATE_KEY` - updater signing key (PEM, encoded as base64)

## Minimal example

The parameters `--config` and `--version` are required to be set.
See [config](config.md) for required config options.

```
# bould main bouf exe
cargo build -r --bin bouf
# run bouf with config
./target/release/bouf -c config.toml --version 29.1.0-beta1
```

## Full help text
```
Usage: bouf [OPTIONS] --config <config.toml> --version <Major.Minor.Patch[-(rc|beta)Num]>

Options:
  -c, --config <config.toml>                        Configuration file
  -v, --version <Major.Minor.Patch[-(rc|beta)Num]>  OBS main version
      --beta <Beta number>                          Beta number
      --rc <RC number>                              RC number
      --branch <Beta branch>                        Branch used in manifest name/update files
      --commit <commit hash>                        Commit hash used in manifest
  -i, --input <new build>                           
  -p, --previous <old builds>                       
  -o, --output <output dir>                         
      --notes-file <file.rtf>                       File containing release notes
      --private-key <file.pem>                      Falls back to "UPDATER_PRIVATE_KEY" env var
      --updater-data-only                           Create only delta patches and manifest
      --skip-installer                              Skip creating NSIS installer
      --skip-patches                                Skip creating delta patches
      --skip-codesigning                            Skip codesigning
      --skip-manifest-signing                       Skip signing manifest
      --clear-output                                Clear existing output directory
  -d, --verbose                                     Verbose logging
  -h, --help                                        Print help (see more with '--help')
  -V, --version                                     Print version
```
