use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[clap(version, about, long_about = "Building OBS Updates Fast(er)")]
pub struct MainArgs {
    // Required
    #[clap(short, long, value_parser, value_name = "Config file")]
    pub config: PathBuf,
    /// OBS main version
    #[clap(short, long, value_parser, value_name = "Major.Minor.Patch[-(rc|beta)Num]")]
    pub version: String,

    // Optional version suffix
    /// Beta number
    #[clap(long, value_parser, value_name = "Beta number")]
    pub beta: Option<u8>,
    /// RC number
    #[clap(long, value_parser, value_name = "RC number")]
    pub rc: Option<u8>,
    /// Branch used in manifest name/update files
    #[clap(long, value_parser, value_name = "Beta branch")]
    pub branch: Option<String>,
    /// Commit hash used in manifest
    #[clap(long, value_parser, value_name = "commit hash")]
    pub commit: Option<String>,

    // Optional overrides
    #[clap(short, long, value_parser, value_name = "new build")]
    pub input: Option<PathBuf>,
    #[clap(short, long, value_parser, value_name = "old builds")]
    pub previous: Option<PathBuf>,
    #[clap(short, long, value_parser, value_name = "output dir")]
    pub output: Option<PathBuf>,
    /// File containing release notes
    #[clap(long, value_parser, value_name = "file.rtf")]
    pub note_file: Option<PathBuf>,
    /// Falls back to "UPDATER_PRIVATE_KEY" env var
    #[clap(long, value_parser, value_name = "file.pem")]
    pub private_key: Option<PathBuf>,

    // Optional flags
    /// Skip creating NSIS installer
    #[clap(long, value_parser, default_value_t = false)]
    pub skip_preparation: bool,
    /// Skip creating NSIS installer
    #[clap(long, value_parser, default_value_t = false)]
    pub skip_installer: bool,
    /// Skip creating delta patches
    #[clap(long, value_parser, default_value_t = false)]
    pub skip_patches: bool,
    /// Skip codesigning
    #[clap(long, value_parser, default_value_t = false)]
    pub skip_codesigning: bool,
    /// Skip signing manifest
    #[clap(long, value_parser, default_value_t = false)]
    pub skip_manifest_signing: bool,
    /// Clear existing output directory
    #[clap(long, value_parser, default_value_t = false)]
    pub clear_output: bool,
}
