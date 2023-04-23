use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = "Building OBS Updates Fast(er)")]
pub struct MainArgs {
    // Required
    /// Configuration file
    #[arg(short, long, value_name = "config.toml")]
    pub config: PathBuf,
    /// OBS main version
    #[arg(short, long, value_name = "Major.Minor.Patch[-(rc|beta)Num]")]
    pub version: String,

    // Optional version suffix
    /// Beta number
    #[arg(long, value_name = "Beta number")]
    pub beta: Option<u8>,
    /// RC number
    #[arg(long, value_name = "RC number")]
    pub rc: Option<u8>,
    /// Branch used in manifest name/update files
    #[arg(long, value_name = "Beta branch")]
    pub branch: Option<String>,
    /// Commit hash used in manifest
    #[arg(
        long,
        value_name = "commit hash",
        conflicts_with = "exclude",
        conflicts_with = "include"
    )]
    pub commit: Option<String>,

    // Optional overrides
    #[arg(short, long, value_name = "new build")]
    pub input: Option<PathBuf>,
    #[arg(short, long, value_name = "old builds")]
    pub previous: Option<PathBuf>,
    #[arg(short, long, value_name = "output dir")]
    pub output: Option<PathBuf>,
    /// File containing release notes
    #[arg(long, value_name = "file.rtf")]
    pub notes_file: Option<PathBuf>,
    /// Falls back to "UPDATER_PRIVATE_KEY" env var
    #[arg(long, value_name = "file.pem")]
    pub private_key: Option<PathBuf>,

    // Optional flags
    /// Create only delta patches and manifest
    #[arg(long, default_value_t = false)]
    pub updater_data_only: bool,
    /// Skip creating NSIS installer
    #[arg(long, default_value_t = false)]
    pub skip_installer: bool,
    /// Skip creating delta patches
    #[arg(long, default_value_t = false)]
    pub skip_patches: bool,
    /// Skip codesigning
    #[arg(long, default_value_t = false)]
    pub skip_codesigning: bool,
    /// Skip signing manifest
    #[arg(long, default_value_t = false)]
    pub skip_manifest_signing: bool,
    /// Clear existing output directory
    #[arg(long, default_value_t = false)]
    pub clear_output: bool,
}
