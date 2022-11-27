use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom, Write};
use std::path::Path;

use anyhow::Result;

use xz2::read::XzDecoder;
use xz2::write::XzEncoder;

use crate::utils::hash::{hash_file, FileInfo};

// 9 | LZMA_PRESET_EXTREME
const LZMA_PRESET: u32 = 9 | 1 << 31;

/// Create OBS-bidiff compatible patch (bidiff + LZMA)
pub fn create_patch(old: &Path, new: &Path, patch: &Path) -> Result<FileInfo> {
    let mut old_file = File::open(old).expect("Unable to open old file");
    let mut new_file = File::open(new).expect("Unable to open new file");
    let mut patch_file = File::create(patch).expect("Unable to open patch file");

    let mut old_buf = Vec::new();
    old_file.read_to_end(&mut old_buf)?;

    let mut new_buf = Vec::new();
    new_file.read_to_end(&mut new_buf)?;

    // Create LZMA writer
    let mut out_data = Cursor::new(Vec::new());
    let mut writer = XzEncoder::new(&mut out_data, LZMA_PRESET);

    bidiff::simple_diff(&old_buf, &new_buf, &mut writer)?;
    writer.finish()?;

    patch_file.write_all(b"BOUF/BIDIFF/LZMA")?;
    patch_file.write_all(&((new_buf.len() as u64).to_le_bytes()))?;
    patch_file.write_all(out_data.get_ref())?;

    Ok(hash_file(patch))
}

/// Apply OBS-bidiff patch
// This function is not implemented in the most memory-efficient way,
// it's only needed for testing though.
#[allow(dead_code)]
pub fn apply_patch(old: &Path, new: &Path, patch: &Path) -> Result<FileInfo> {
    let old_file = File::open(old).expect("Unable to open old file");
    let patch_file = File::open(patch).expect("Unable to open patch file");
    let mut new_file = File::create(new).expect("Unable to open new file");

    let mut old_reader = BufReader::new(old_file);
    let mut patch_reader = BufReader::new(patch_file);
    // Skip header
    patch_reader.seek(SeekFrom::Start(16))?;
    // Read size of output file
    let mut size_buf = [0; 8];
    patch_reader.read_exact(&mut size_buf)?;
    let size = u64::from_le_bytes(size_buf) as usize;
    // Create LZMA reader
    let reader = XzDecoder::new(&mut patch_reader);
    // Create new buffer and patch it
    let mut new_buf = vec![0; size];

    let mut r = bipatch::Reader::new(reader, &mut old_reader)?;
    let _read = r.read(&mut new_buf)?;
    new_file.write_all(&new_buf)?;

    Ok(hash_file(new))
}

#[cfg(test)]
mod bidiff_tests {
    use super::*;

    #[test]
    fn test_bidiff() {
        // First, create the patch
        let old = Path::new("extra/test_files/in.txt");
        let new = Path::new("extra/test_files/out.txt");
        let patch = Path::new("extra/test_files/patch_bidiff.bin");
        let patch_info = create_patch(old, new, patch).unwrap();
        assert_eq!(patch_info.hash, "4226ea4d16e4dd9df13840d90bd64918f0f7a1e6");

        // Try applying the patch
        let out = Path::new("extra/test_files/out_test_bidiff.txt");
        let res = apply_patch(old, out, patch).unwrap();
        assert_eq!(res.hash, "50b242bcef918cc8363e9cf1a27a1420928948e9");
    }
}
