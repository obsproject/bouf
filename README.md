# BOUF - Building OBS Updates Fast(er)

`bouf` is the application used for preparing and building OBS Studio Windows release distribution packages and updater delta patches.

See [docs](docs/README.md) for more information.

## Usage

See [docs/cli](docs/cli.md) and [docs/config](docs/config.md) for usage details.

## License

The source code found in `src/` is licensed under Apache-2 (see `LICENSE.txt`).

Files in `extra/nsis` may have other licenses and exist primarily for CI usage and testing,
and may not be redistributed under the Apache-2 terms.

# ToDo

- Go through older code and replace `.expect()`s and `panic!`s with anyhow errors 
  + This will require some larger changes in some codepaths, do this later...
- Figure out how to deal with nightlies
  + Disable copy to previous directory?
  + No deltas to avoid problems?
- Make zstd level configurable
