#![allow(clippy::unwrap_used)]

use diskdiet::config::{self, Config};
use diskdiet::disk::free_bytes;
use diskdiet::log;
use diskdiet::model::{Action, Category, Item, Removal, Risk};
use diskdiet::scan::{ScanContext, compare_items, inherit_child_risk, is_kept, scan_all};
use diskdiet::size::{disk_usage, format_bytes};
use diskdiet::time;
use diskdiet::tools::{Tools, find_on_path, last_absolute_path, path_from_login_shell_output};
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use tempfile::tempdir;

const BLOCK_BYTES: u64 = 512;
const PAYLOAD_BYTES: usize = 1_000_000;

fn write_payload(path: &Path) {
    fs::write(path, vec![7u8; PAYLOAD_BYTES]).unwrap();
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
fn disk_usage_counts_real_allocation() {
    let dir = tempdir().unwrap();
    write_payload(&dir.path().join("a.bin"));
    let usage = disk_usage(&[dir.path().to_path_buf()]);
    assert!(usage >= PAYLOAD_BYTES as u64);
    assert_eq!(usage % BLOCK_BYTES, 0);
}

#[test]
fn disk_usage_counts_a_hardlinked_file_once() {
    let dir = tempdir().unwrap();
    let original = dir.path().join("original.bin");
    write_payload(&original);
    let single = disk_usage(&[dir.path().to_path_buf()]);
    fs::hard_link(&original, dir.path().join("link1.bin")).unwrap();
    fs::hard_link(&original, dir.path().join("link2.bin")).unwrap();
    assert_eq!(disk_usage(&[dir.path().to_path_buf()]), single);
}

#[test]
fn disk_usage_dedupes_hardlinks_across_separate_roots() {
    let dir = tempdir().unwrap();
    let (first, second) = (dir.path().join("first"), dir.path().join("second"));
    fs::create_dir_all(&first).unwrap();
    fs::create_dir_all(&second).unwrap();
    write_payload(&first.join("data.bin"));
    fs::hard_link(first.join("data.bin"), second.join("data.bin")).unwrap();
    let both = disk_usage(&[first.clone(), second.clone()]);
    let alone = disk_usage(&[first]) + disk_usage(&[second]);
    assert!(both < alone);
}

#[test]
fn disk_usage_does_not_follow_symlinks() {
    let outside = tempdir().unwrap();
    write_payload(&outside.path().join("big.bin"));
    let dir = tempdir().unwrap();
    symlink(outside.path(), dir.path().join("dir_link")).unwrap();
    symlink(outside.path().join("big.bin"), dir.path().join("file_link")).unwrap();
    assert!(disk_usage(&[dir.path().to_path_buf()]) < PAYLOAD_BYTES as u64);
}

#[test]
fn disk_usage_does_not_follow_a_symlinked_root() {
    let outside = tempdir().unwrap();
    write_payload(&outside.path().join("big.bin"));
    let dir = tempdir().unwrap();
    let link = dir.path().join("root_link");
    symlink(outside.path(), &link).unwrap();
    assert!(disk_usage(&[link]) < PAYLOAD_BYTES as u64);
}

#[test]
fn disk_usage_includes_hidden_files_and_ignores_missing_paths() {
    let dir = tempdir().unwrap();
    write_payload(&dir.path().join(".hidden.bin"));
    let missing = dir.path().join("does-not-exist");
    assert!(disk_usage(&[dir.path().to_path_buf(), missing]) >= PAYLOAD_BYTES as u64);
}

#[test]
fn disk_usage_of_a_single_file_root() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("one.bin");
    write_payload(&file);
    assert!(disk_usage(&[file]) >= PAYLOAD_BYTES as u64);
}

const MANY_ENTRIES: usize = 3_000;
const KIB: u64 = 1024;

fn du_bytes(path: &Path) -> u64 {
    let out = std::process::Command::new("du")
        .arg("-sk")
        .arg(path)
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    text.split_whitespace()
        .next()
        .unwrap()
        .parse::<u64>()
        .unwrap()
        * KIB
}

#[test]
fn disk_usage_matches_du_across_many_entries_and_nested_folders() {
    let dir = tempdir().unwrap();
    let nested = dir.path().join("a/b/c");
    fs::create_dir_all(&nested).unwrap();
    for index in 0..MANY_ENTRIES {
        fs::write(
            dir.path()
                .join(format!("file-with-a-long-name-{index}.txt")),
            "x",
        )
        .unwrap();
    }
    write_payload(&nested.join("deep.bin"));
    assert_eq!(
        disk_usage(&[dir.path().to_path_buf()]),
        du_bytes(dir.path())
    );
}

#[test]
fn disk_usage_skips_unreadable_folders_without_failing() {
    let dir = tempdir().unwrap();
    let locked = dir.path().join("locked");
    fs::create_dir(&locked).unwrap();
    write_payload(&locked.join("hidden.bin"));
    write_payload(&dir.path().join("visible.bin"));
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let usage = disk_usage(&[dir.path().to_path_buf()]);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(usage >= PAYLOAD_BYTES as u64 && usage < 2 * PAYLOAD_BYTES as u64);
}

#[test]
fn free_bytes_is_nonzero_for_the_temp_dir() {
    let dir = tempdir().unwrap();
    assert!(free_bytes(dir.path()).unwrap() > 0);
}

#[test]
fn free_bytes_errors_for_a_missing_path() {
    assert!(free_bytes(Path::new("/definitely/not/here")).is_err());
}

#[test]
fn config_defaults_match_the_plan() {
    let config = Config::default();
    assert!(config.dev_roots.is_empty());
    assert!(config.exclude.is_empty());
    assert!(config.extra_cache_dirs.is_empty());
    assert_eq!(config.stale_app_days, 90);
    assert_eq!(config.stale_download_days, 90);
    assert_eq!(config.stale_node_modules_days, 30);
    assert_eq!(config.docker_prune_until_hours, 720);
    assert_eq!(config.low_space_warning_gb, 100);
}

#[test]
fn default_path_lives_under_dot_config() {
    let home = Path::new("/home/x");
    assert_eq!(
        config::default_path(home),
        PathBuf::from("/home/x/.config/diskdiet/config.toml")
    );
}

#[test]
fn default_dev_roots_returns_only_existing_folders() {
    let home = tempdir().unwrap();
    fs::create_dir(home.path().join("Develop")).unwrap();
    fs::create_dir(home.path().join("code")).unwrap();
    fs::write(home.path().join("src"), "a file, not a folder").unwrap();
    let roots = config::default_dev_roots(home.path());
    assert_eq!(
        roots,
        vec![home.path().join("Develop"), home.path().join("code")]
    );
}

#[test]
fn load_without_any_file_gives_defaults_with_discovered_roots() {
    let home = tempdir().unwrap();
    fs::create_dir(home.path().join("Projects")).unwrap();
    let loaded = config::load(None, home.path()).unwrap();
    assert_eq!(loaded.dev_roots, vec![home.path().join("Projects")]);
    assert_eq!(loaded.stale_app_days, 90);
}

#[test]
fn load_reads_toml_and_expands_tilde() {
    let home = tempdir().unwrap();
    let file = home.path().join("custom.toml");
    fs::write(
        &file,
        "dev_roots = [\"~/work\", \"/abs/place\"]\nexclude = [\"~\"]\nextra_cache_dirs = [\"~/.cache/foo\"]\nstale_app_days = 10\n",
    )
    .unwrap();
    let loaded = config::load(Some(&file), home.path()).unwrap();
    assert_eq!(
        loaded.dev_roots,
        vec![home.path().join("work"), PathBuf::from("/abs/place")]
    );
    assert_eq!(loaded.exclude, vec![home.path().to_path_buf()]);
    assert_eq!(
        loaded.extra_cache_dirs,
        vec![home.path().join(".cache/foo")]
    );
    assert_eq!(loaded.stale_app_days, 10);
    assert_eq!(loaded.stale_download_days, 90);
}

#[test]
fn load_reads_the_default_file_when_present() {
    let home = tempdir().unwrap();
    let file = config::default_path(home.path());
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "dev_roots = [\"~/x\"]\nstale_app_days = 3\n").unwrap();
    let loaded = config::load(None, home.path()).unwrap();
    assert_eq!(loaded.dev_roots, vec![home.path().join("x")]);
    assert_eq!(loaded.stale_app_days, 3);
}

#[test]
fn load_falls_back_to_default_roots_when_dev_roots_is_empty() {
    let home = tempdir().unwrap();
    fs::create_dir(home.path().join("dev")).unwrap();
    let file = home.path().join("c.toml");
    fs::write(&file, "dev_roots = []\n").unwrap();
    let loaded = config::load(Some(&file), home.path()).unwrap();
    assert_eq!(loaded.dev_roots, vec![home.path().join("dev")]);
}

#[test]
fn load_fails_for_a_missing_explicit_file_and_for_bad_toml() {
    let home = tempdir().unwrap();
    assert!(config::load(Some(&home.path().join("nope.toml")), home.path()).is_err());
    let bad = home.path().join("bad.toml");
    fs::write(&bad, "stale_app_days = \"many\"").unwrap();
    assert!(config::load(Some(&bad), home.path()).is_err());
}

#[test]
fn load_rejects_misspelled_fields_and_relative_paths() {
    let home = tempdir().unwrap();
    let typo = home.path().join("typo.toml");
    fs::write(&typo, "exlude = [\"~/keep\"]").unwrap();
    assert!(config::load(Some(&typo), home.path()).is_err());
    let relative = home.path().join("relative.toml");
    fs::write(&relative, "dev_roots = [\"code\"]").unwrap();
    assert!(config::load(Some(&relative), home.path()).is_err());
}

#[test]
fn disk_usage_counts_allocated_blocks_of_a_sparse_file() {
    let dir = tempdir().unwrap();
    let sparse = fs::File::create(dir.path().join("disk.img")).unwrap();
    sparse.set_len(10 * PAYLOAD_BYTES as u64).unwrap();
    let usage = disk_usage(&[dir.path().to_path_buf()]);
    assert_eq!(usage, du_bytes(dir.path()));
    assert!(usage < PAYLOAD_BYTES as u64);
}

#[test]
fn login_shell_output_yields_the_marked_path_despite_rc_noise() {
    let noisy = "Welcome back!\n\n__DISKDIET_PATH__=/opt/homebrew/bin:/usr/bin\nbye\n";
    assert_eq!(
        path_from_login_shell_output(noisy),
        Some(std::ffi::OsString::from("/opt/homebrew/bin:/usr/bin"))
    );
    assert_eq!(path_from_login_shell_output("no marker here"), None);
}

#[test]
fn expand_tilde_only_touches_a_leading_tilde_component() {
    let home = Path::new("/h");
    assert_eq!(
        config::expand_tilde(Path::new("~"), home),
        PathBuf::from("/h")
    );
    assert_eq!(
        config::expand_tilde(Path::new("~/a/b"), home),
        PathBuf::from("/h/a/b")
    );
    assert_eq!(
        config::expand_tilde(Path::new("~bob/a"), home),
        PathBuf::from("~bob/a")
    );
    assert_eq!(
        config::expand_tilde(Path::new("/x/~/y"), home),
        PathBuf::from("/x/~/y")
    );
}

#[test]
fn log_record_appends_timestamped_lines_and_creates_folders() {
    let home = tempdir().unwrap();
    log::record(home.path(), "delete /tmp/a -> Done");
    log::record(home.path(), "multi\nline");
    let text = fs::read_to_string(log::log_path(home.path())).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].ends_with("  delete /tmp/a -> Done"));
    assert!(lines[1].ends_with("  multi line"));
    assert_eq!(lines[0].as_bytes()[4], b'-');
    assert_eq!(lines[0].as_bytes()[13], b':');
}

#[test]
fn format_timestamp_handles_known_instants() {
    assert_eq!(time::format_timestamp(0), "1970-01-01 00:00:00");
    assert_eq!(time::format_timestamp(951_827_696), "2000-02-29 12:34:56");
    assert_eq!(time::format_timestamp(1_767_225_599), "2025-12-31 23:59:59");
}

#[test]
fn find_on_path_requires_an_executable_regular_file() {
    let dir = tempdir().unwrap();
    let tool = dir.path().join("mytool");
    fs::write(&tool, "#!/bin/sh\n").unwrap();
    let path_var = std::env::join_paths([dir.path()]).unwrap();
    assert_eq!(find_on_path("mytool", &path_var), None);
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(find_on_path("mytool", &path_var), Some(tool));
}

#[test]
fn last_absolute_path_skips_terminal_noise() {
    let dir = tempdir().unwrap();
    let real = dir.path().join("pnpm");
    fs::write(&real, "").unwrap();
    let noisy = format!(
        "\u{1b}]1337;ShellIntegration\u{7}welcome banner\n\u{1b}[0m{}\u{1b}[0m\r\nshell: trailing text\n",
        real.display()
    );
    assert_eq!(last_absolute_path(&noisy), Some(real));
    assert_eq!(last_absolute_path("alias pnpm=foo\n/no/such/file\n"), None);
}

#[test]
fn tools_from_map_and_get_roundtrip() {
    let map = [("git".to_string(), PathBuf::from("/usr/bin/git"))].into();
    let tools = Tools::from_map(map);
    assert_eq!(tools.get("git"), Some(Path::new("/usr/bin/git")));
    assert_eq!(tools.get("nope"), None);
}

#[test]
fn tools_discover_finds_git_and_never_invents_tools() {
    let tools = Tools::discover();
    assert!(tools.get("git").is_some_and(Path::exists));
    assert_eq!(tools.get("definitely-not-a-tool"), None);
}

fn item(category: Category, risk: Risk, bytes: u64, action: Action) -> Item {
    Item {
        category,
        label: format!("{category:?}-{risk:?}-{bytes}"),
        detail: String::new(),
        bytes,
        risk,
        action,
        preselected: false,
        typed_confirmation: None,
    }
}

#[test]
fn scan_all_runs_every_scanner_over_an_empty_home() {
    let home = tempdir().unwrap();
    let config = Config {
        dev_roots: vec![home.path().join("none")],
        ..Config::default()
    };
    let ctx = ScanContext {
        home: home.path().to_path_buf(),
        config,
        tools: Tools::from_map(Default::default()),
    };
    let items = scan_all(&ctx);
    assert!(
        items
            .windows(2)
            .all(|pair| pair[0].category <= pair[1].category)
    );
}

const BIG: u64 = 5_000_000;

fn delete_item(bytes: u64, path: &str) -> Item {
    item(
        Category::BuildArtifacts,
        Risk::Safe,
        bytes,
        Action::DeletePath(PathBuf::from(path)),
    )
}

#[test]
fn is_kept_drops_items_below_one_megabyte() {
    assert!(!is_kept(&delete_item(0, "/a"), &[]));
    assert!(!is_kept(&delete_item(999_999, "/a"), &[]));
    assert!(is_kept(&delete_item(1_000_000, "/a"), &[]));
    let other = Action::RunCommand {
        program: PathBuf::from("/bin/ls"),
        args: vec![],
    };
    assert!(!is_kept(&item(Category::Docker, Risk::Safe, 0, other), &[]));
}

#[test]
fn is_kept_ignores_size_floor_for_worktrees_trash_and_fstrim() {
    let prune = Action::PruneWorktrees {
        repo: PathBuf::from("/r"),
    };
    assert!(is_kept(
        &item(Category::AgentWorktrees, Risk::Safe, 0, prune),
        &[]
    ));
    assert!(is_kept(
        &item(Category::Trash, Risk::Review, 0, Action::EmptyTrash),
        &[]
    ));
    let trim = Action::RunCommand {
        program: PathBuf::from("/usr/local/bin/colima"),
        args: ["ssh", "--", "sudo", "fstrim", "-a"]
            .map(String::from)
            .to_vec(),
    };
    assert!(is_kept(&item(Category::Docker, Risk::Safe, 0, trim), &[]));
}

#[test]
fn is_kept_keeps_tiny_worktrees() {
    let worktree = Action::RemoveWorktree {
        repo: PathBuf::from("/r"),
        worktree: PathBuf::from("/r/wt"),
        removal: Removal::Rechecked,
    };
    assert!(is_kept(
        &item(Category::AgentWorktrees, Risk::Danger, 1, worktree),
        &[]
    ));
}

#[test]
fn is_kept_honours_exclude_in_both_directions() {
    let exclude = [PathBuf::from("/home/me/keep")];
    assert!(!is_kept(
        &delete_item(BIG, "/home/me/keep/proj/target"),
        &exclude
    ));
    assert!(!is_kept(&delete_item(BIG, "/home/me/keep"), &exclude));
    assert!(!is_kept(&delete_item(BIG, "/home/me"), &exclude));
    assert!(is_kept(
        &delete_item(BIG, "/home/me/keeper/target"),
        &exclude
    ));
    assert!(is_kept(&delete_item(BIG, "/home/other/target"), &exclude));
}

#[test]
fn is_kept_matches_exclusions_regardless_of_case() {
    let exclude = [PathBuf::from("/home/me/develop/keep")];
    assert!(!is_kept(
        &delete_item(BIG, "/home/me/Develop/Keep/target"),
        &exclude
    ));
}

#[test]
fn a_safe_parent_inherits_the_risk_of_an_item_inside_it() {
    let mut parent = delete_item(BIG, "/home/me/.cache");
    parent.preselected = true;
    let child = item(
        Category::AiModels,
        Risk::Review,
        BIG,
        Action::DeletePath(PathBuf::from("/home/me/.cache/huggingface")),
    );
    let mut items = vec![parent, child];
    inherit_child_risk(&mut items);
    assert_eq!((items[0].risk, items[0].preselected), (Risk::Review, false));
    assert_eq!(items[1].risk, Risk::Review);
}

fn sort_key(items: &[Item]) -> Vec<(Category, Risk, u64)> {
    items
        .iter()
        .map(|i| (i.category, i.risk, i.bytes))
        .collect()
}

#[test]
fn compare_items_sorts_by_category_then_risk_then_bytes_descending() {
    let build = |category, risk, bytes| item(category, risk, bytes, Action::EmptyTrash);
    let mut items = [
        build(Category::Trash, Risk::Safe, 1),
        build(Category::Docker, Risk::Review, 50),
        build(Category::Docker, Risk::Safe, 5),
        build(Category::Docker, Risk::Safe, 7),
        build(Category::AgentWorktrees, Risk::Danger, 2),
    ];
    items.sort_by(compare_items);
    let expected = [
        (Category::AgentWorktrees, Risk::Danger, 2),
        (Category::Docker, Risk::Safe, 7),
        (Category::Docker, Risk::Safe, 5),
        (Category::Docker, Risk::Review, 50),
        (Category::Trash, Risk::Safe, 1),
    ];
    assert_eq!(sort_key(&items), expected);
}
