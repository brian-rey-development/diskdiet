use anyhow::{Result, anyhow};
use diskdiet::domain::config::{self, Config};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

#[test]
fn default_config_has_empty_lists_and_stock_thresholds() {
    let config = Config::default();
    assert_eq!(config.dev_roots, [] as [std::path::PathBuf; 0]);
    assert_eq!(config.exclude, [] as [std::path::PathBuf; 0]);
    assert_eq!(config.extra_cache_dirs, [] as [std::path::PathBuf; 0]);
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
fn default_dev_roots_returns_only_existing_folders() -> Result<()> {
    let home = tempdir()?;
    fs::create_dir(home.path().join("Develop"))?;
    fs::create_dir(home.path().join("code"))?;
    fs::write(home.path().join("src"), "a file, not a folder")?;
    let roots = config::default_dev_roots(home.path());
    assert_eq!(
        roots,
        vec![home.path().join("Develop"), home.path().join("code")]
    );
    Ok(())
}

#[test]
fn load_without_any_file_gives_defaults_with_discovered_roots() -> Result<()> {
    let home = tempdir()?;
    fs::create_dir(home.path().join("Projects"))?;
    let loaded = config::load(None, home.path())?;
    assert_eq!(loaded.dev_roots, vec![home.path().join("Projects")]);
    assert_eq!(loaded.stale_app_days, 90);
    Ok(())
}

#[test]
fn load_reads_toml_and_expands_tilde() -> Result<()> {
    let home = tempdir()?;
    let file = home.path().join("custom.toml");
    fs::write(
        &file,
        "dev_roots = [\"~/work\", \"/abs/place\"]\nexclude = [\"~\"]\nextra_cache_dirs = [\"~/.cache/foo\"]\nstale_app_days = 10\n",
    )?;
    let loaded = config::load(Some(&file), home.path())?;
    assert_eq!(
        loaded.dev_roots,
        vec![home.path().join("work"), PathBuf::from("/abs/place")]
    );
    assert_eq!(loaded.exclude.first(), Some(&home.path().to_path_buf()));
    assert_eq!(
        loaded.extra_cache_dirs,
        vec![home.path().join(".cache/foo")]
    );
    assert_eq!(loaded.stale_app_days, 10);
    assert_eq!(loaded.stale_download_days, 90);
    Ok(())
}

#[test]
fn load_reads_the_default_file_when_present() -> Result<()> {
    let home = tempdir()?;
    let file = config::default_path(home.path());
    let folder = file.parent().ok_or_else(|| anyhow!("no parent"))?;
    fs::create_dir_all(folder)?;
    fs::write(&file, "dev_roots = [\"~/x\"]\nstale_app_days = 3\n")?;
    let loaded = config::load(None, home.path())?;
    assert_eq!(loaded.dev_roots, vec![home.path().join("x")]);
    assert_eq!(loaded.stale_app_days, 3);
    Ok(())
}

#[test]
fn load_falls_back_to_default_roots_when_dev_roots_is_empty() -> Result<()> {
    let home = tempdir()?;
    fs::create_dir(home.path().join("dev"))?;
    let file = home.path().join("c.toml");
    fs::write(&file, "dev_roots = []\n")?;
    let loaded = config::load(Some(&file), home.path())?;
    assert_eq!(loaded.dev_roots, vec![home.path().join("dev")]);
    Ok(())
}

#[test]
fn load_fails_for_a_missing_explicit_file_and_for_bad_toml() -> Result<()> {
    let home = tempdir()?;
    assert!(config::load(Some(&home.path().join("nope.toml")), home.path()).is_err());
    let bad = home.path().join("bad.toml");
    fs::write(&bad, "stale_app_days = \"many\"")?;
    assert!(config::load(Some(&bad), home.path()).is_err());
    Ok(())
}

#[test]
fn load_rejects_misspelled_fields_and_relative_paths() -> Result<()> {
    let home = tempdir()?;
    let typo = home.path().join("typo.toml");
    fs::write(&typo, "exlude = [\"~/keep\"]")?;
    assert!(config::load(Some(&typo), home.path()).is_err());
    let relative = home.path().join("relative.toml");
    fs::write(&relative, "dev_roots = [\"code\"]")?;
    assert!(config::load(Some(&relative), home.path()).is_err());
    Ok(())
}

#[test]
fn expand_tilde_only_touches_a_leading_tilde_component() {
    let home = Path::new("/h");
    let expand = |text: &str| config::expand_tilde(Path::new(text), home);
    assert_eq!(expand("~"), PathBuf::from("/h"));
    assert_eq!(expand("~/a/b"), PathBuf::from("/h/a/b"));
    assert_eq!(expand("~bob/a"), PathBuf::from("~bob/a"));
    assert_eq!(expand("/x/~/y"), PathBuf::from("/x/~/y"));
}

#[test]
fn exclude_also_matches_the_resolved_path_behind_a_symlink() -> Result<()> {
    let home = tempdir()?;
    let project = home.path().join("real/project");
    fs::create_dir_all(&project)?;
    std::os::unix::fs::symlink(home.path().join("real"), home.path().join("link"))?;
    let file = home.path().join("custom.toml");
    fs::write(&file, "exclude = [\"~/link/project\"]\n")?;
    let loaded = config::load(Some(&file), home.path())?;
    assert!(loaded.exclude.contains(&home.path().join("link/project")));
    assert!(loaded.exclude.contains(&fs::canonicalize(&project)?));
    Ok(())
}

#[test]
fn without_aliases_drops_a_second_name_for_the_same_folder() -> Result<()> {
    let home = tempdir()?;
    let real = home.path().join("real");
    fs::create_dir(&real)?;
    let alias = home.path().join("alias");
    std::os::unix::fs::symlink(&real, &alias)?;
    let missing = home.path().join("missing");
    let kept = config::without_aliases(vec![real.clone(), alias, missing.clone()]);
    assert_eq!(kept, vec![real, missing]);
    Ok(())
}
