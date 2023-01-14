use std::fs::File;
use std::io;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;

use anyhow::Result;

use zstd::stream::{Decoder, Encoder};

use crate::utils::hash::{hash_file, FileInfo};

// 3 = default, 19 = normal max, 22 = extreme
const ZSTD_LEVEL: i32 = 22;

/// Create delta based on ZSTD dictionary
pub fn create_patch(old: &Path, new: &Path, patch: &Path) -> Result<FileInfo> {
    let mut old_file = File::open(old).expect("Unable to open old file");
    let mut new_file = File::open(new).expect("Unable to open new file");
    let mut patch_file = File::create(patch).expect("Unable to open patch file");

    let new_size = new_file.metadata()?.len();
    let mut old_buf = Vec::new();
    old_file.read_to_end(&mut old_buf)?;

    // Create ZSTD writer wtih old file as dictionary
    let mut out_data = Vec::<u8>::new();
    let mut writer = Encoder::with_dictionary(&mut out_data, ZSTD_LEVEL, &old_buf)?;

    // Compress and finalise
    io::copy(&mut new_file, &mut writer)?;
    writer.finish()?;

    patch_file.write_all(b"BOUF//ZSTD//DICT")?;
    patch_file.write_all(&new_size.to_le_bytes())?;
    patch_file.write_all(&out_data)?;

    Ok(hash_file(patch))
}

/// Apply OBS-zstd patch
// This function is not implemented in the most memory-efficient way,
// it's only needed for testing though.
#[allow(dead_code)]
pub fn apply_patch(old: &Path, new: &Path, patch: &Path) -> Result<FileInfo> {
    let mut old_file = File::open(old).expect("Unable to open old file");
    let patch_file = File::open(patch).expect("Unable to open patch file");
    let mut new_file = File::create(new).expect("Unable to open new file");

    let mut patch_reader = BufReader::new(patch_file);

    let mut old_buf = Vec::new();
    old_file.read_to_end(&mut old_buf)?;
    // Skip header
    patch_reader.seek(SeekFrom::Start(16))?;
    // Read size of output file
    let mut size_buf = [0; 8];
    patch_reader.read_exact(&mut size_buf)?;
    let size = u64::from_le_bytes(size_buf) as usize;
    // Create new buffer and patch it
    let mut new_buf = Vec::new();

    let mut decoder = Decoder::with_dictionary(&mut patch_reader, &old_buf)?;
    io::copy(&mut decoder, &mut new_buf)?;

    if new_buf.len() != size {
        panic!("Output size incorrect! {} != {}", new_buf.len(), size)
    }

    new_file.write_all(&new_buf)?;

    Ok(hash_file(new))
}

#[cfg(test)]
mod zstd_tests {
    use super::*;

    #[test]
    fn test_zstd() {
        // First, create the patch
        let old = Path::new("extra/test_files/in.txt");
        let new = Path::new("extra/test_files/out.txt");
        let patch = Path::new("extra/test_files/patch_zstd.bin");
        let patch_info = create_patch(old, new, patch).unwrap();
        assert_eq!(patch_info.hash, "af3a20c95bb3988bc0770196e390d2cbd10af904");

        // Try applying the patch
        let out = Path::new("extra/test_files/out_test_zstd.txt");
        let res = apply_patch(old, out, patch).unwrap();
        assert_eq!(res.hash, "50b242bcef918cc8363e9cf1a27a1420928948e9");
    }
}
