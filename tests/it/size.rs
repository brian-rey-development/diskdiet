use crate::support::fixtures::{set_mode, write_file};
use crate::support::{BLOCK_BYTES, NO_ACCESS_MODE, RWX_MODE};
use anyhow::{Result, anyhow};
use diskdiet::size::{disk_usage, format_bytes};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

const PAYLOAD_BYTES: usize = 1_000_000;
const PAYLOAD_SIZE: u64 = PAYLOAD_BYTES as u64;
const MANY_ENTRIES: usize = 3_000;
const KIB: u64 = 1024;

fn write_payload(path: &Path) -> Result<()> {
    Ok(fs::write(path, vec![7u8; PAYLOAD_BYTES])?)
}

fn du_bytes(path: &Path) -> Result<u64> {
    let output = Command::new("du").arg("-sk").arg(path).output()?;
    let text = String::from_utf8(output.stdout)?;
    let kib = text
        .split_whitespace()
        .next()
        .ok_or_else(|| anyhow!("du printed nothing"))?;
    Ok(kib.parse::<u64>()? * KIB)
}

#[test]
fn format_bytes_uses_finder_style_decimal_units() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(999), "999 B");
    assert_eq!(format_bytes(12_000), "12 KB");
    assert_eq!(format_bytes(845_000_000), "845 MB");
    assert_eq!(format_bytes(1_200_000_000), "1.2 GB");
    assert_eq!(format_bytes(23_500_000), "23.5 MB");
    assert_eq!(format_bytes(287_000_000_000), "287 GB");
    assert_eq!(format_bytes(2_500_000_000_000), "2.5 TB");
}

#[test]
fn format_bytes_never_prints_a_thousand_of_a_unit() {
    assert_eq!(format_bytes(999_600), "1.0 MB");
    assert_eq!(format_bytes(u64::MAX), "18446744 TB");
    assert_eq!(format_bytes(99_950_000), "100 MB");
    assert_eq!(format_bytes(99_940_000), "99.9 MB");
}

#[test]
fn disk_usage_counts_real_allocation() -> Result<()> {
    let dir = tempdir()?;
    write_payload(&dir.path().join("a.bin"))?;
    let usage = disk_usage(&[dir.path().to_path_buf()]);
    assert!(usage >= PAYLOAD_SIZE);
    assert_eq!(usage % BLOCK_BYTES, 0);
    Ok(())
}

#[test]
fn disk_usage_counts_a_hardlinked_file_once() -> Result<()> {
    let dir = tempdir()?;
    let original = dir.path().join("original.bin");
    write_payload(&original)?;
    let single = disk_usage(&[dir.path().to_path_buf()]);
    fs::hard_link(&original, dir.path().join("link1.bin"))?;
    fs::hard_link(&original, dir.path().join("link2.bin"))?;
    assert_eq!(disk_usage(&[dir.path().to_path_buf()]), single);
    Ok(())
}

#[test]
fn disk_usage_dedupes_hardlinks_across_separate_roots() -> Result<()> {
    let dir = tempdir()?;
    let (first, second) = (dir.path().join("first"), dir.path().join("second"));
    fs::create_dir_all(&first)?;
    fs::create_dir_all(&second)?;
    write_payload(&first.join("data.bin"))?;
    fs::hard_link(first.join("data.bin"), second.join("data.bin"))?;
    let both = disk_usage(&[first.clone(), second.clone()]);
    let alone = disk_usage(&[first]) + disk_usage(&[second]);
    assert!(both < alone);
    Ok(())
}

#[test]
fn disk_usage_does_not_follow_symlinks() -> Result<()> {
    let outside = tempdir()?;
    write_payload(&outside.path().join("big.bin"))?;
    let dir = tempdir()?;
    symlink(outside.path(), dir.path().join("dir_link"))?;
    symlink(outside.path().join("big.bin"), dir.path().join("file_link"))?;
    assert!(disk_usage(&[dir.path().to_path_buf()]) < PAYLOAD_SIZE);
    Ok(())
}

#[test]
fn disk_usage_does_not_follow_a_symlinked_root() -> Result<()> {
    let outside = tempdir()?;
    write_payload(&outside.path().join("big.bin"))?;
    let dir = tempdir()?;
    let link = dir.path().join("root_link");
    symlink(outside.path(), &link)?;
    assert!(disk_usage(&[link]) < PAYLOAD_SIZE);
    Ok(())
}

#[test]
fn disk_usage_includes_hidden_files_and_ignores_missing_paths() -> Result<()> {
    let dir = tempdir()?;
    write_payload(&dir.path().join(".hidden.bin"))?;
    let missing = dir.path().join("does-not-exist");
    assert!(disk_usage(&[dir.path().to_path_buf(), missing]) >= PAYLOAD_SIZE);
    Ok(())
}

#[test]
fn disk_usage_of_a_single_file_root() -> Result<()> {
    let dir = tempdir()?;
    let file = dir.path().join("one.bin");
    write_payload(&file)?;
    assert!(disk_usage(&[file]) >= PAYLOAD_SIZE);
    Ok(())
}

#[test]
fn disk_usage_matches_du_across_many_entries_and_nested_folders() -> Result<()> {
    let dir = tempdir()?;
    let nested = dir.path().join("a/b/c");
    fs::create_dir_all(&nested)?;
    for index in 0..MANY_ENTRIES {
        let name = format!("file-with-a-long-name-{index}.txt");
        write_file(&dir.path().join(name), "x")?;
    }
    write_payload(&nested.join("deep.bin"))?;
    assert_eq!(
        disk_usage(&[dir.path().to_path_buf()]),
        du_bytes(dir.path())?
    );
    Ok(())
}

#[test]
fn disk_usage_skips_unreadable_folders_without_failing() -> Result<()> {
    let dir = tempdir()?;
    let locked = dir.path().join("locked");
    fs::create_dir(&locked)?;
    write_payload(&locked.join("hidden.bin"))?;
    write_payload(&dir.path().join("visible.bin"))?;
    set_mode(&locked, NO_ACCESS_MODE)?;
    let usage = disk_usage(&[dir.path().to_path_buf()]);
    set_mode(&locked, RWX_MODE)?;
    assert!((PAYLOAD_SIZE..2 * PAYLOAD_SIZE).contains(&usage));
    Ok(())
}

#[test]
fn disk_usage_counts_allocated_blocks_of_a_sparse_file() -> Result<()> {
    let dir = tempdir()?;
    let sparse = fs::File::create(dir.path().join("disk.img"))?;
    sparse.set_len(10 * PAYLOAD_SIZE)?;
    let usage = disk_usage(&[dir.path().to_path_buf()]);
    assert_eq!(usage, du_bytes(dir.path())?);
    assert!(usage < PAYLOAD_SIZE);
    Ok(())
}
