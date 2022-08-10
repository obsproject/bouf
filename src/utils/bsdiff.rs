use std::fs::File;
use std::io::Cursor;
use std::io::Result;
use std::io::{Read, Write};
use std::path::Path;

use bsdiff::diff::diff;
use xz2::write::XzEncoder;

use crate::utils::hash::{hash_file, FileInfo};

const LZMA_PRESET: u32 = 9 | 1 << 31;

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

    patch_file.write(b"JIMSLEY/BSDIFF43")?;
    patch_file.write(&((new_buf.len() as u64).to_le_bytes()))?;
    patch_file.write_all(&out_data.get_ref())?;

    Ok(hash_file(patch))
}

#[cfg(test)]
mod bsdiff_tests {
    use super::*;
    use crate::utils::hash::hash_file;
    use std::io::Cursor;

    #[test]
    fn test_diff() {
        let old = Path::new("extra/test_files/in.txt");
        let new = Path::new("extra/test_files/out.txt");
        let patch = Path::new("extra/test_files/patch.bin");
        let patch_info = create_patch(old, new, patch).unwrap();
        assert_eq!(patch_info.hash, "cc44d732f2f07d39fa556c2d7336da73e1671783");
    }
}
