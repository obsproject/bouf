use std::fs::File;
use std::io::{BufReader, Cursor, Read, Result, Seek, SeekFrom, Write};
use std::path::Path;

use bsdiff::diff;
use bsdiff::patch as bspatch;
use xz2::read::XzDecoder;
use xz2::write::XzEncoder;

use crate::utils::hash::{hash_file, FileInfo};

// 9 | LZMA_PRESET_EXTREME
const LZMA_PRESET: u32 = 9 | 1 << 31;

/// Create OBS-bsdiff compatible patch file (bsdiff + LZMA)
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

    diff(&old_buf, &new_buf, &mut writer)?;
    writer.finish()?;

    patch_file.write_all(b"JIMSLEY/BSDIFF43")?;
    patch_file.write_all(&((new_buf.len() as u64).to_le_bytes()))?;
    patch_file.write_all(out_data.get_ref())?;

    Ok(hash_file(patch))
}

/// Apply OBS-bsdiff patch
// This function is not implemented in the most memory-efficient way,
// it's only needed for testing though.
#[allow(dead_code)]
pub fn apply_patch(old: &Path, new: &Path, patch: &Path) -> Result<FileInfo> {
    let mut old_file = File::open(old).expect("Unable to open old file");
    let mut new_file = File::create(new).expect("Unable to open new file");
    let patch_file = File::open(patch).expect("Unable to open patch file");

    let mut old_buf = Vec::new();
    old_file.read_to_end(&mut old_buf)?;

    let mut patch_data = BufReader::new(patch_file);
    // Skip header
    patch_data.seek(SeekFrom::Start(16))?;
    // Read size of output file
    let mut size_buf = [0; 8];
    patch_data.read_exact(&mut size_buf)?;
    let size = u64::from_le_bytes(size_buf) as usize;
    // Create LZMA reader
    let mut reader = XzDecoder::new(&mut patch_data);
    // Create new buffer and patch it
    let mut new_buf = vec![0; size];
    bspatch(&old_buf, &mut reader, &mut new_buf)?;
    new_file.write_all(&new_buf)?;

    Ok(hash_file(new))
}

#[cfg(test)]
mod bsdiff_tests {
    use super::*;

    #[test]
    fn test_diff() {
        // First, create the patch
        let old = Path::new("extra/test_files/in.txt");
        let new = Path::new("extra/test_files/out.txt");
        let patch = Path::new("extra/test_files/patch.bin");
        let patch_info = create_patch(old, new, patch).unwrap();
        assert_eq!(patch_info.hash, "cc44d732f2f07d39fa556c2d7336da73e1671783");

        // Try applying the patch
        let out = Path::new("extra/test_files/out_test.txt");
        let res = apply_patch(old, out, patch).unwrap();
        assert_eq!(res.hash, "50b242bcef918cc8363e9cf1a27a1420928948e9");
    }
}
