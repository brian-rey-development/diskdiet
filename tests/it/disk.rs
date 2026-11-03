use anyhow::Result;
use diskdiet::disk::free_bytes;
use std::path::Path;
use tempfile::tempdir;

#[test]
fn free_bytes_is_nonzero_for_the_temp_dir() -> Result<()> {
    let dir = tempdir()?;
    assert!(free_bytes(dir.path())? > 0);
    Ok(())
}

#[test]
fn free_bytes_errors_for_a_missing_path() {
    assert!(free_bytes(Path::new("/definitely/not/here")).is_err());
}
