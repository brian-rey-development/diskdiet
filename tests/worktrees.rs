mod common;

use anyhow::{Result, anyhow};
use common::{add_worktree, commit_file, init_repo, temp_root, test_ctx, write_file};
use diskdiet::git::ignored::is_build_output;
use diskdiet::git::porcelain::parse_porcelain;
use diskdiet::git::status::count_status;
use diskdiet::model::{Action, Category, Item, Removal, Risk};
use diskdiet::scan::worktrees::{Assessment, assess, base_branch, find_repos, inspect, scan};
use std::fs;
use std::path::{Path, PathBuf};

const GIT: &str = "git";

fn inspect_worktree(wt: &Path) -> Result<Assessment> {
    inspect(Path::new(GIT), wt, Some("main")).ok_or_else(|| anyhow!("inspect failed"))
}

fn repo_with_worktree(root: &Path, branch: &str) -> Result<(PathBuf, PathBuf)> {
    let repo = root.join("repo");
    init_repo(&repo)?;
    let wt = add_worktree(&repo, &root.join(format!("wt-{branch}")), branch)?;
    Ok((repo, wt))
}

fn scan_items(home: &Path, roots: Vec<PathBuf>) -> Vec<Item> {
    scan(&test_ctx(home, roots))
}

fn item_for<'a>(items: &'a [Item], branch: &str) -> Result<&'a Item> {
    let found = items.iter().find(|item| item.label == branch);
    found.ok_or_else(|| anyhow!("no item labelled {branch}"))
}

#[test]
fn zero_commits_ahead_with_uncommitted_changes_is_danger() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (_repo, wt) = repo_with_worktree(&root, "feature")?;
    write_file(&wt.join("notes.txt"), "unsaved thoughts")?;
    let assessment = inspect_worktree(&wt)?;
    assert_eq!((assessment.ahead, assessment.dirty), (Some(0), 1));
    assert_eq!(assessment.risk(), Risk::Danger);
    Ok(())
}

#[test]
fn danger_worktree_requires_typed_delete_and_is_not_preselected() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (_repo, wt) = repo_with_worktree(&root, "feature")?;
    write_file(&wt.join("notes.txt"), "unsaved thoughts")?;
    let items = scan_items(&root, vec![root.clone()]);
    let item = item_for(&items, "feature")?;
    assert_eq!(item.risk, Risk::Danger);
    assert_eq!(item.typed_confirmation.as_deref(), Some("delete"));
    assert!(!item.preselected);
    assert_eq!(item.category, Category::AgentWorktrees);
    Ok(())
}

#[test]
fn ignored_env_file_alone_makes_a_worktree_danger() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (_repo, wt) = repo_with_worktree(&root, "feature")?;
    write_file(&wt.join(".env"), "SECRET=1")?;
    let assessment = inspect_worktree(&wt)?;
    assert_eq!((assessment.dirty, assessment.risky_ignored), (0, 1));
    assert_eq!(assessment.risk(), Risk::Danger);
    Ok(())
}

#[test]
fn ignored_personal_folder_makes_a_worktree_danger() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (repo, wt) = repo_with_worktree(&root, "feature")?;
    write_file(&repo.join(".git/info/exclude"), "local-data/\n")?;
    write_file(&wt.join("local-data/app.sqlite"), "rows")?;
    let assessment = inspect_worktree(&wt)?;
    assert_eq!((assessment.dirty, assessment.risky_ignored), (0, 1));
    assert_eq!(assessment.risk(), Risk::Danger);
    Ok(())
}

#[test]
fn ignored_build_output_alone_keeps_a_worktree_safe() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (_repo, wt) = repo_with_worktree(&root, "feature")?;
    write_file(&wt.join("target/debug/app"), "binary")?;
    write_file(&wt.join("node_modules/dep/index.js"), "module")?;
    let assessment = inspect_worktree(&wt)?;
    assert_eq!((assessment.dirty, assessment.risky_ignored), (0, 0));
    assert_eq!(assessment.risk(), Risk::Safe);
    Ok(())
}

#[test]
fn safe_worktree_is_preselected_in_scan() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (repo, wt) = repo_with_worktree(&root, "feature")?;
    write_file(&wt.join("target/debug/app"), "binary")?;
    let items = scan_items(&root, vec![root.clone()]);
    let item = item_for(&items, "feature")?;
    assert_eq!((item.risk, item.preselected), (Risk::Safe, true));
    assert!(item.typed_confirmation.is_none());
    assert_eq!(
        item.action,
        Action::RemoveWorktree {
            repo: fs::canonicalize(repo)?,
            worktree: wt,
            removal: Removal::Rechecked,
        }
    );
    assert!(item.detail.starts_with("merged · 0 ahead"));
    Ok(())
}

#[test]
fn commits_ahead_with_a_clean_tree_is_review() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (_repo, wt) = repo_with_worktree(&root, "feature")?;
    commit_file(&wt, "feature.txt", "work", "add feature")?;
    let assessment = inspect_worktree(&wt)?;
    assert_eq!((assessment.ahead, assessment.dirty), (Some(1), 0));
    assert_eq!(assessment.risk(), Risk::Review);
    let items = scan_items(&root, vec![root.clone()]);
    let item = item_for(&items, "feature")?;
    assert_eq!((item.risk, item.preselected), (Risk::Review, false));
    Ok(())
}

#[test]
fn deleted_worktree_folders_yield_one_prune_item_per_repo() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (repo, first) = repo_with_worktree(&root, "first")?;
    let second = add_worktree(&repo, &root.join("wt-second"), "second")?;
    fs::remove_dir_all(&first)?;
    fs::remove_dir_all(&second)?;
    let items = scan_items(&root, vec![root.clone()]);
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].action,
        Action::PruneWorktrees {
            repo: fs::canonicalize(repo)?
        }
    );
    assert_eq!(
        (items[0].bytes, items[0].risk, items[0].preselected),
        (0, Risk::Safe, true)
    );
    Ok(())
}

#[test]
fn worktrees_outside_the_dev_roots_are_followed() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (_elsewhere, outside) = temp_root()?;
    let repo = root.join("dev/repo");
    init_repo(&repo)?;
    add_worktree(&repo, &outside.join("orca/workspaces/feature"), "feature")?;
    let items = scan_items(&root, vec![root.join("dev")]);
    assert_eq!(item_for(&items, "feature")?.risk, Risk::Safe);
    Ok(())
}

#[test]
fn main_and_detached_worktrees_are_not_offered() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let repo = root.join("repo");
    init_repo(&repo)?;
    let detached = root.join("wt-detached");
    common::git(
        &repo,
        &["worktree", "add", "--detach", &detached.to_string_lossy()],
    )?;
    assert!(scan_items(&root, vec![root.clone()]).is_empty());
    Ok(())
}

#[test]
fn base_branch_falls_back_to_the_first_known_default() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let repo = root.join("repo");
    init_repo(&repo)?;
    assert_eq!(
        base_branch(Path::new(GIT), &repo).as_deref(),
        Some("refs/heads/main")
    );
    Ok(())
}

#[test]
fn repos_inside_agent_worktree_folders_are_not_rediscovered() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let repo = root.join("repo");
    init_repo(&repo)?;
    add_worktree(&repo, &repo.join(".claude/worktrees/agent"), "agent")?;
    let found = find_repos(std::slice::from_ref(&root), &[]);
    assert_eq!(found, vec![fs::canonicalize(repo)?]);
    Ok(())
}

#[test]
fn porcelain_parser_handles_spaces_detached_and_bare_entries() {
    let text = "worktree /Users/me/My Repo\nHEAD aaa\nbranch refs/heads/main\n\n\
                worktree /Users/me/wt one\nHEAD bbb\nbranch refs/heads/feat/x\n\n\
                worktree /Users/me/wt-detached\nHEAD ccc\ndetached\n\n\
                worktree /srv/bare.git\nbare\n\n\
                worktree /Volumes/x/gone\nHEAD ddd\nbranch refs/heads/gone\n\
                locked on a usb drive\nprunable gitdir file points to non-existent location\n";
    let entries = parse_porcelain(text);
    assert_eq!(entries.len(), 5);
    assert_eq!(entries[0].path, PathBuf::from("/Users/me/My Repo"));
    assert_eq!(entries[1].path, PathBuf::from("/Users/me/wt one"));
    assert_eq!(entries[1].branch.as_deref(), Some("feat/x"));
    assert!(entries[2].detached && entries[2].branch.is_none());
    assert!(entries[3].bare && entries[3].head.is_none());
    assert!(entries[4].locked && entries[4].prunable);
}

#[test]
fn porcelain_parser_ignores_empty_input_and_quoted_paths() {
    assert!(parse_porcelain("").is_empty());
    assert!(parse_porcelain("\n\n").is_empty());
    assert!(parse_porcelain("worktree \"/odd\\npath\"\nHEAD a\n").is_empty());
}

#[test]
fn assess_combines_ahead_dirty_and_ignored_counts() {
    assert_eq!(assess(0, 0, 0), Risk::Safe);
    assert_eq!(assess(3, 0, 0), Risk::Review);
    assert_eq!(assess(0, 1, 0), Risk::Danger);
    assert_eq!(assess(0, 0, 1), Risk::Danger);
    assert_eq!(assess(5, 2, 1), Risk::Danger);
}

#[test]
fn build_output_is_recognised_by_any_path_component() {
    for known in [
        "target/",
        "apps/web/node_modules/",
        ".next/",
        ".venv/",
        "x/__pycache__/",
        "apps/web/next-env.d.ts",
        "apps/web/tsconfig.tsbuildinfo",
        ".husky/_/",
        ".husky/_/pre-commit",
        "coverage/",
        "apps/web/.tanstack/",
        "app/src-tauri/gen/",
    ] {
        assert!(is_build_output(known), "{known}");
    }
}

#[test]
fn personal_files_are_never_build_output() {
    for personal in [
        ".env",
        "config/.env.local",
        "secrets/key.pem",
        "targets/a",
        "notes.md",
        ".husky/pre-commit",
        ".vercel/project.json",
        "src/gen/schema.rs",
        "pkg/dist/a.js",
        "deploy/build/signing.pem",
        "target",
    ] {
        assert!(!is_build_output(personal), "{personal}");
    }
}

#[test]
fn status_counts_split_dirty_from_risky_ignored() {
    let text = " M src/a.rs\0?? new.txt\0!! target/\0!! .env\0!! web/node_modules/\0R  renamed.rs\0old name.rs\0";
    let counts = count_status(text);
    assert_eq!((counts.dirty, counts.risky_ignored), (3, 1));
    assert_eq!(count_status(""), Default::default());
}

#[test]
fn malformed_status_fields_count_as_dirty() {
    assert_eq!(count_status("X\0").dirty, 1);
}

#[test]
fn skip_worktree_edits_make_a_worktree_danger() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (_repo, wt) = repo_with_worktree(&root, "feature")?;
    common::git(&wt, &["update-index", "--skip-worktree", "README.md"])?;
    write_file(&wt.join("README.md"), "local only edit")?;
    let assessment = inspect_worktree(&wt)?;
    assert_eq!(assessment.dirty, 1);
    assert_eq!(assessment.risk(), Risk::Danger);
    Ok(())
}

fn add_real_submodule(root: &Path, wt: &Path) -> Result<()> {
    let lib = root.join("lib");
    init_repo(&lib)?;
    let source = lib.to_string_lossy().into_owned();
    let allow = ["-c", "protocol.file.allow=always"];
    common::git(
        wt,
        &[&allow[..], &["submodule", "add", "-q", &source, "lib"]].concat(),
    )?;
    common::git(wt, &["commit", "-m", "add submodule"]).map(drop)
}

#[test]
fn worktrees_with_submodules_are_danger_because_git_needs_force() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (_repo, wt) = repo_with_worktree(&root, "feature")?;
    add_real_submodule(&root, &wt)?;
    let assessment =
        inspect(Path::new(GIT), &wt, Some("main")).ok_or_else(|| anyhow!("inspect"))?;
    assert!(assessment.has_submodules);
    assert_eq!(assessment.risk(), Risk::Danger);
    let item = item_for(&scan_items(&root, vec![root.join("repo")]), "feature")?.clone();
    assert!(matches!(
        item.action,
        Action::RemoveWorktree {
            removal: Removal::Forced,
            ..
        }
    ));
    Ok(())
}

#[test]
fn no_prune_is_offered_while_a_worktree_sits_on_an_unplugged_volume() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (repo, first) = repo_with_worktree(&root, "first")?;
    let unplugged = add_worktree(&repo, &root.join("wt-second"), "second")?;
    fs::remove_dir_all(&first)?;
    fs::remove_dir_all(&unplugged)?;
    let gitdir = repo.join(".git/worktrees/wt-second/gitdir");
    write_file(&gitdir, "/Volumes/diskdiet-test-unplugged/wt-second/.git\n")?;
    assert!(scan_items(&root, vec![root.clone()]).is_empty());
    Ok(())
}
