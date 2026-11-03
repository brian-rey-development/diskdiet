use anyhow::{Result, anyhow};
use diskdiet::freedesktop_trash::{deletion_date_now, empty, encode_path, info_text, move_to};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

const DELETED_AT: &str = "2026-11-03T10:00:00";
const DELETION_DATE_LEN: usize = "2026-01-02T03:04:05".len();
const DATE_TIME_SEPARATOR_INDEX: usize = 10;

#[test]
fn deletion_date_uses_a_t_separator_and_no_spaces() {
    let date = deletion_date_now();
    assert_eq!(date.len(), DELETION_DATE_LEN);
    assert_eq!(date.as_bytes()[DATE_TIME_SEPARATOR_INDEX], b'T');
    assert!(!date.contains(' '));
}

#[test]
fn encoded_paths_percent_encode_spaces_and_non_ascii_bytes() {
    assert_eq!(encode_path(Path::new("/a b/é")), "/a%20b/%C3%A9");
    assert_eq!(encode_path(Path::new("/keep-_.~/ok")), "/keep-_.~/ok");
}

#[test]
fn trash_info_follows_the_freedesktop_format() {
    let text = info_text(Path::new("/home/me/My Notes/ñ.txt"), DELETED_AT);
    assert_eq!(
        text,
        "[Trash Info]\nPath=/home/me/My%20Notes/%C3%B1.txt\nDeletionDate=2026-11-03T10:00:00\n"
    );
    assert_eq!(encode_path(Path::new("/a-b_c.d~e")), "/a-b_c.d~e");
}

#[test]
fn moving_to_the_trash_keeps_the_file_and_records_where_it_came_from() -> Result<()> {
    let home = tempdir()?;
    let trash = home.path().join("Trash");
    let file = home.path().join("report.pdf");
    fs::write(&file, "pdf")?;
    move_to(&trash, &file, DELETED_AT)?;
    assert!(!file.exists());
    assert_eq!(fs::read_to_string(trash.join("files/report.pdf"))?, "pdf");
    let info = fs::read_to_string(trash.join("info/report.pdf.trashinfo"))?;
    assert!(info.contains(&format!("Path={}", encode_path(&file))));
    Ok(())
}

#[test]
fn a_second_file_with_the_same_name_gets_a_new_trash_name() -> Result<()> {
    let home = tempdir()?;
    let trash = home.path().join("Trash");
    for folder in ["a", "b"] {
        let file = home.path().join(folder).join("setup.deb");
        fs::create_dir_all(file.parent().ok_or_else(|| anyhow!("parent"))?)?;
        fs::write(&file, folder)?;
        move_to(&trash, &file, DELETED_AT)?;
    }
    assert_eq!(fs::read_to_string(trash.join("files/setup.deb"))?, "a");
    assert_eq!(fs::read_to_string(trash.join("files/setup.deb.2"))?, "b");
    assert!(trash.join("info/setup.deb.2.trashinfo").exists());
    Ok(())
}

#[test]
fn emptying_the_trash_clears_files_info_and_expunged() -> Result<()> {
    let home = tempdir()?;
    let trash = home.path().join("Trash");
    let folder = home.path().join("old-project");
    fs::create_dir_all(folder.join("src"))?;
    fs::write(folder.join("src/main.rs"), "fn main() {}")?;
    move_to(&trash, &folder, DELETED_AT)?;
    fs::create_dir_all(trash.join("expunged/leftover"))?;
    fs::write(trash.join("directorysizes"), "cache")?;
    empty(&trash)?;
    for dir in ["files", "info", "expunged"] {
        assert_eq!(fs::read_dir(trash.join(dir))?.count(), 0, "{dir}");
    }
    assert!(!trash.join("directorysizes").exists());
    Ok(())
}

#[test]
fn emptying_a_trash_that_does_not_exist_is_fine() -> Result<()> {
    let home = tempdir()?;
    empty(&home.path().join("never-created"))?;
    Ok(())
}

#[test]
fn emptying_the_trash_removes_the_directory_size_cache() -> Result<()> {
    let trash = tempdir()?;
    fs::write(trash.path().join("directorysizes"), "1 2 name\n")?;
    empty(trash.path())?;
    assert!(!trash.path().join("directorysizes").exists());
    Ok(())
}
