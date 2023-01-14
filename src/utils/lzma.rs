use std::fs::File;
use std::io;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;

use anyhow::Result;
use xz2::read::XzDecoder;
use xz2::stream;
use xz2::write::XzEncoder;

use crate::utils::hash::{hash_file, FileInfo};

// 9 | LZMA_PRESET_EXTREME
const LZMA_PRESET: u32 = 9 | 1 << 31;

/// Create OBS-bsdiff compatible patch file (bsdiff + LZMA)
pub fn create_patch(_old: &Path, new: &Path, patch: &Path) -> Result<FileInfo> {
    let mut new_file = File::open(new).expect("Unable to open new file");
    let mut patch_file = File::create(patch).expect("Unable to open patch file");

    // Set up LZMA options and filters
    let mut _opts = stream::LzmaOptions::new_preset(LZMA_PRESET)?;
    let mut _filters = stream::Filters::new();

    let opts = _opts.dict_size(64 * 1024 * 1024).nice_len(128);
    let filters = _filters.x86().lzma2(opts);

    let stream = stream::Stream::new_stream_encoder(filters, stream::Check::None)?;
    // let stream = stream::MtStreamBuilder::new().threads(8).filters(_filters).block_size(64*1024*1024).encoder()?;

    // Create LZMA writer
    let mut writer = XzEncoder::new_stream(&mut patch_file, stream);
    io::copy(&mut new_file, &mut writer)?;
    writer.finish()?;

    Ok(hash_file(patch))
}

/// Apply OBS-bsdiff patch
// This function is not implemented in the most memory-efficient way,
// it's only needed for testing though.
#[allow(dead_code)]
pub fn apply_patch(_old: &Path, new: &Path, patch: &Path) -> Result<FileInfo> {
    let mut new_file = File::create(new).expect("Unable to open new file");
    let patch_file = File::open(patch).expect("Unable to open patch file");

    let mut patch_data = BufReader::new(patch_file);
    // Create LZMA reader
    let mut reader = XzDecoder::new(&mut patch_data);
    io::copy(&mut reader, &mut new_file)?;

    Ok(hash_file(new))
}

/// Taken from bsdiff-rs/src/patch.rs
#[inline]
fn offtin(buf: [u8; 8]) -> i64 {
    let y = i64::from_le_bytes(buf);
    if 0 == y & (1 << 63) {
        y
    } else {
        -(y & !(1 << 63))
    }
}

#[cfg(test)]
mod lzma_tests {
    use super::*;

    #[test]
    fn test_lzma() {
        // First, create the patch
        let old = Path::new("extra/test_files/in.txt");
        let new = Path::new("extra/test_files/out.txt");
        let patch = Path::new("extra/test_files/patch_lzma.bin");
        let patch_info = create_patch(old, new, patch).unwrap();
        assert_eq!(patch_info.hash, "e4c27c5bf271e92a5f6256e81285e75f831ad791");

        // Try applying the patch
        let out = Path::new("extra/test_files/out_test_lzma.txt");
        let res = apply_patch(old, out, patch).unwrap();
        assert_eq!(res.hash, "50b242bcef918cc8363e9cf1a27a1420928948e9");
    }
}
