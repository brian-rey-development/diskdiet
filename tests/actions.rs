#![allow(clippy::expect_used, clippy::unwrap_used)]

use diskdiet::actions::{
    applescript_escape, execute, is_safe_delete_target, perform, prune_names, trash_script,
};
use diskdiet::model::{Action, Category, Item, Outcome, Risk};
use diskdiet::tools::Tools;
use diskdiet::ui::exec::execution_order;
use diskdiet::ui::pick::{confirm_prompt, item_label, starts_selected};
use diskdiet::ui::report::report_json;
use diskdiet::ui::share::{boxed, percent_encode, tweet_url, tweet_url_for_label};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn item(action: Action) -> Item {
    Item {
        category: Category::BuildArtifacts,
        label: "fixture".to_string(),
        detail: "fixture detail".to_string(),
        bytes: 1024,
        risk: Risk::Safe,
        action,
        preselected: true,
        typed_confirmation: None,
    }
}

fn git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=test", "-c", "user.email=test@example.com"])
        .args(["-c", "commit.gpgsign=false", "-C"])
        .arg(repo)
        .args(args)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?} failed");
}

fn repo_with_worktree(root: &Path) -> (PathBuf, PathBuf) {
    let repo = root.join("repo");
    let worktree = root.join("feature-wt");
    fs::create_dir(&repo).expect("repo dir");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]);
    let wt = worktree.to_string_lossy().to_string();
    git(&repo, &["worktree", "add", "-q", "-b", "feature", &wt]);
    (repo, worktree)
}

fn branch_exists(repo: &Path, name: &str) -> bool {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["branch", "--list", name])
        .output()
        .expect("git runs");
    !String::from_utf8_lossy(&output.stdout).trim().is_empty()
}

struct RestorePerms(PathBuf);

impl Drop for RestorePerms {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o755));
    }
}

struct DryFixture {
    dir: PathBuf,
    file: PathBuf,
    marker: PathBuf,
    repo: PathBuf,
    worktree: PathBuf,
}

fn dry_fixture(root: &Path) -> DryFixture {
    let dir = root.join("victim-dir");
    let file = root.join("victim.txt");
    fs::create_dir(&dir).expect("dir");
    fs::write(&file, "x").expect("file");
    let (repo, worktree) = repo_with_worktree(root);
    DryFixture {
        dir,
        file,
        marker: root.join("marker"),
        repo,
        worktree,
    }
}

fn every_variant(fx: &DryFixture) -> Vec<Action> {
    vec![
        Action::DeletePath(fx.dir.clone()),
        Action::DeletePath(fx.file.clone()),
        Action::MoveToTrash(fx.file.clone()),
        Action::EmptyTrash,
        Action::RunCommand {
            program: PathBuf::from("/usr/bin/touch"),
            args: vec![fx.marker.to_string_lossy().to_string()],
        },
        Action::RemoveWorktree {
            repo: fx.repo.clone(),
            worktree: fx.worktree.clone(),
            force: true,
        },
        Action::PruneWorktrees {
            repo: fx.repo.clone(),
        },
    ]
}

#[test]
fn dry_run_never_touches_files_for_every_variant() {
    let temp = TempDir::new().expect("tempdir");
    let fx = dry_fixture(temp.path());
    for action in every_variant(&fx) {
        let outcome = execute(&item(action), &Tools::default(), true);
        assert_eq!(outcome, Outcome::DryRun);
    }
    assert!(fx.dir.exists() && fx.file.exists() && fx.worktree.exists());
    assert!(!fx.marker.exists());
}

#[test]
fn delete_path_removes_a_directory_tree_and_a_file() {
    let temp = TempDir::new().expect("tempdir");
    let dir = temp.path().join("tree");
    fs::create_dir_all(dir.join("a/b")).expect("tree");
    fs::write(dir.join("a/b/file"), "data").expect("file");
    let file = temp.path().join("single.txt");
    fs::write(&file, "data").expect("file");
    assert_eq!(
        perform(&Action::DeletePath(dir.clone()), &Tools::default()),
        Outcome::Done
    );
    assert_eq!(
        perform(&Action::DeletePath(file.clone()), &Tools::default()),
        Outcome::Done
    );
    assert!(!dir.exists());
    assert!(!file.exists());
}

#[test]
fn delete_path_does_not_follow_symlinks() {
    let temp = TempDir::new().expect("tempdir");
    let target = temp.path().join("precious");
    fs::create_dir(&target).expect("dir");
    fs::write(target.join("keep"), "x").expect("file");
    let link = temp.path().join("link");
    std::os::unix::fs::symlink(&target, &link).expect("symlink");
    assert_eq!(
        perform(&Action::DeletePath(link.clone()), &Tools::default()),
        Outcome::Done
    );
    assert!(!link.exists());
    assert!(target.join("keep").exists());
}

#[test]
fn delete_path_on_missing_path_is_skipped() {
    let temp = TempDir::new().expect("tempdir");
    let outcome = perform(
        &Action::DeletePath(temp.path().join("nope")),
        &Tools::default(),
    );
    assert!(matches!(outcome, Outcome::Skipped(_)));
}

#[test]
fn delete_path_refuses_root_and_relative_paths() {
    for path in ["/", "/Users", "relative/dir"] {
        let outcome = perform(&Action::DeletePath(PathBuf::from(path)), &Tools::default());
        assert!(
            matches!(outcome, Outcome::Failed(_)),
            "{path} must be refused"
        );
    }
}

#[test]
fn delete_guard_protects_home_its_ancestors_and_shared_folders() {
    let home = Some(Path::new("/Users/me"));
    for refused in [
        "/Users/me",
        "/users/ME",
        "/Users",
        "/Users/me/Documents",
        "/Users/me/.cache",
        "/Users/someone",
        "/Volumes/Backup",
        "/Users/me/x/../..",
        "/Users/me/./Documents",
    ] {
        assert!(
            !is_safe_delete_target(Path::new(refused), home),
            "{refused}"
        );
    }
}

#[test]
fn delete_guard_allows_real_cleanup_targets() {
    let home = Some(Path::new("/Users/me"));
    for allowed in [
        "/Users/me/.cache/puppeteer",
        "/Users/me/Develop/app/target",
        "/Applications/Old.app",
        "/Volumes/Work/app/target",
    ] {
        assert!(is_safe_delete_target(Path::new(allowed), home), "{allowed}");
    }
}

#[test]
fn delete_path_in_read_only_parent_needs_full_disk_access() {
    let temp = TempDir::new().expect("tempdir");
    let locked = temp.path().join("locked");
    fs::create_dir(&locked).expect("dir");
    let victim = locked.join("victim");
    fs::write(&victim, "x").expect("file");
    let _restore = RestorePerms(locked.clone());
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).expect("chmod");
    let outcome = perform(&Action::DeletePath(victim.clone()), &Tools::default());
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).expect("restore");
    assert_eq!(outcome, Outcome::NeedsFullDiskAccess);
    assert!(victim.exists());
}

#[test]
fn run_command_failure_is_reported_and_success_is_done() {
    let fail = Action::RunCommand {
        program: PathBuf::from("/usr/bin/false"),
        args: vec![],
    };
    let pass = Action::RunCommand {
        program: PathBuf::from("/usr/bin/true"),
        args: vec![],
    };
    assert!(matches!(
        perform(&fail, &Tools::default()),
        Outcome::Failed(_)
    ));
    assert_eq!(perform(&pass, &Tools::default()), Outcome::Done);
}

#[test]
fn run_command_failure_carries_the_stderr_tail() {
    let action = Action::RunCommand {
        program: PathBuf::from("/bin/sh"),
        args: vec!["-c".to_string(), "echo boom >&2; exit 3".to_string()],
    };
    assert_eq!(
        perform(&action, &Tools::default()),
        Outcome::Failed("boom".to_string())
    );
}

#[test]
fn run_command_with_missing_program_fails_without_panicking() {
    let action = Action::RunCommand {
        program: PathBuf::from("/nonexistent/tool"),
        args: vec![],
    };
    assert!(matches!(
        perform(&action, &Tools::default()),
        Outcome::Failed(_)
    ));
}

fn remove(repo: &Path, worktree: &Path, force: bool) -> Outcome {
    let action = Action::RemoveWorktree {
        repo: repo.to_path_buf(),
        worktree: worktree.to_path_buf(),
        force,
    };
    perform(&action, &Tools::default())
}

#[test]
fn remove_worktree_removes_the_folder_and_keeps_the_branch() {
    let temp = TempDir::new().expect("tempdir");
    let (repo, worktree) = repo_with_worktree(temp.path());
    assert_eq!(remove(&repo, &worktree, false), Outcome::Done);
    assert!(!worktree.exists());
    assert!(branch_exists(&repo, "feature"));
}

#[test]
fn unconfirmed_worktree_that_changed_since_the_scan_is_kept() {
    let temp = TempDir::new().expect("tempdir");
    let (repo, worktree) = repo_with_worktree(temp.path());
    fs::write(worktree.join("scratch.txt"), "written after the scan").expect("file");
    assert!(matches!(
        remove(&repo, &worktree, false),
        Outcome::Skipped(_)
    ));
    assert!(worktree.join("scratch.txt").exists());
}

#[test]
fn typed_confirmation_removes_a_dirty_worktree() {
    let temp = TempDir::new().expect("tempdir");
    let (repo, worktree) = repo_with_worktree(temp.path());
    fs::write(worktree.join("scratch.txt"), "uncommitted").expect("dirty file");
    assert_eq!(remove(&repo, &worktree, true), Outcome::Done);
    assert!(!worktree.exists());
}

#[test]
fn prune_preview_lines_name_the_worktrees_git_would_forget() {
    let stderr = "Removing worktrees/gone: gitdir file points to non-existent location\n\
                  Removing worktrees/old one: not a valid directory\n\
                  something else\n";
    assert_eq!(prune_names(stderr), vec!["gone", "old one"]);
}

#[test]
fn prune_is_skipped_when_it_would_forget_a_worktree_on_an_unplugged_drive() {
    let temp = TempDir::new().expect("tempdir");
    let (repo, worktree) = repo_with_worktree(temp.path());
    fs::remove_dir_all(&worktree).expect("remove folder");
    let name = worktree
        .file_name()
        .expect("name")
        .to_string_lossy()
        .into_owned();
    let gitdir = repo.join(".git/worktrees").join(&name).join("gitdir");
    fs::write(&gitdir, "/Volumes/diskdiet-test-unplugged/wt/.git\n").expect("gitdir");
    let action = Action::PruneWorktrees { repo: repo.clone() };
    assert!(matches!(
        perform(&action, &Tools::default()),
        Outcome::Skipped(_)
    ));
    assert!(gitdir.exists());
}

#[test]
fn prune_worktrees_clears_entries_whose_folder_is_gone() {
    let temp = TempDir::new().expect("tempdir");
    let (repo, worktree) = repo_with_worktree(temp.path());
    fs::remove_dir_all(&worktree).expect("remove folder");
    let action = Action::PruneWorktrees { repo: repo.clone() };
    assert_eq!(perform(&action, &Tools::default()), Outcome::Done);
    let list = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["worktree", "list"])
        .output()
        .expect("git");
    assert!(!String::from_utf8_lossy(&list.stdout).contains("feature-wt"));
    assert!(branch_exists(&repo, "feature"));
}

#[test]
fn applescript_escape_handles_quotes_and_backslashes() {
    assert_eq!(applescript_escape(r#"a"b"#), r#"a\"b"#);
    assert_eq!(applescript_escape(r"a\b"), r"a\\b");
    assert_eq!(applescript_escape(r#"a\"b"#), r#"a\\\"b"#);
    assert_eq!(applescript_escape("/plain/path"), "/plain/path");
}

#[test]
fn trash_script_wraps_the_escaped_path() {
    let script = trash_script(Path::new(r#"/Applications/My "App"\x.app"#));
    assert_eq!(
        script,
        r#"tell application "Finder" to delete POSIX file "/Applications/My \"App\"\\x.app""#
    );
}

#[test]
fn percent_encode_keeps_unreserved_and_escapes_the_rest() {
    assert_eq!(percent_encode("AZaz09-_.~"), "AZaz09-_.~");
    assert_eq!(percent_encode("a b\n&/:"), "a%20b%0A%26%2F%3A");
    assert_eq!(percent_encode("é"), "%C3%A9");
}

#[test]
fn tweet_url_for_label_is_a_single_encoded_intent_link() {
    let url = tweet_url_for_label("287 GB");
    let machine = percent_encode(diskdiet::platform::MACHINE);
    let start = format!(
        "https://x.com/intent/post?text=I%20just%20freed%20287%20GB%20on%20my%20{machine}%20with"
    );
    assert!(url.starts_with(&start), "{url}");
    assert!(url.contains("%0A%0Ahttps%3A%2F%2Fgithub.com%2Fbrian-rey-development%2Fdiskdiet"));
    assert!(!url.contains(' ') && !url.contains('\n'));
}

#[test]
fn tweet_url_formats_the_freed_bytes() {
    let url = tweet_url(1_500_000_000);
    assert!(url.contains("1.5%20GB"), "{url}");
}

#[test]
fn execution_order_empties_the_trash_first_and_trims_the_vm_last() {
    let run = |args: &[&str]| Action::RunCommand {
        program: PathBuf::from("/usr/bin/true"),
        args: args.iter().map(ToString::to_string).collect(),
    };
    let trash = item(Action::EmptyTrash);
    let trim = item(run(&["ssh", "--", "sudo", "fstrim", "-a"]));
    let prune = item(run(&["image", "prune", "-af"]));
    let app = item(Action::MoveToTrash(PathBuf::from("/Applications/X.app")));
    let ordered = execution_order(&[&trash, &trim, &prune, &app]);
    let actions: Vec<&Action> = ordered.iter().map(|i| &i.action).collect();
    assert_eq!(
        actions,
        vec![&trash.action, &prune.action, &app.action, &trim.action]
    );
}

#[test]
fn danger_items_are_marked_and_never_start_selected() {
    let mut danger = item(Action::EmptyTrash);
    danger.risk = Risk::Danger;
    danger.typed_confirmation = Some("delete".to_string());
    assert!(!starts_selected(&danger));
    assert!(item_label(&danger).contains("DANGER"));
    assert!(starts_selected(&item(Action::EmptyTrash)));
}

#[test]
fn confirm_prompt_wording_changes_in_dry_run() {
    assert_eq!(
        confirm_prompt(3, 0, true),
        "Dry run: nothing will be deleted. Continue?"
    );
    assert!(confirm_prompt(3, 0, false).starts_with("Delete 3 items ("));
}

#[test]
fn report_json_lists_every_item_with_its_risk() {
    let mut risky = item(Action::EmptyTrash);
    risky.risk = Risk::Danger;
    let text = report_json(&[item(Action::EmptyTrash), risky]).expect("json");
    let value: serde_json::Value = serde_json::from_str(&text).expect("valid json");
    assert_eq!(value.as_array().map(Vec::len), Some(2));
    assert_eq!(value[0]["risk"], "safe");
    assert_eq!(value[1]["risk"], "danger");
    assert_eq!(value[0]["bytes"], 1024);
}

#[test]
fn boxed_card_has_equal_width_lines() {
    let lines = vec!["short".to_string(), "a much longer line".to_string()];
    let card = boxed(&lines);
    let widths: Vec<usize> = card.lines().map(|l| l.chars().count()).collect();
    assert!(
        widths.windows(2).all(|pair| pair[0] == pair[1]),
        "{widths:?}"
    );
}
