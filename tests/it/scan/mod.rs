mod apps;
mod builds;
mod caches;
mod docker;
mod downloads;
mod models;
mod pipeline;
mod worktrees;

use crate::support::fixtures::{test_ctx, write_file};
use anyhow::Result;
use diskdiet::model::{Category, Risk};
use diskdiet::scan::{scan_all, scan_only};
use tempfile::tempdir;

const MODEL_BYTES: usize = 1_200_000;

#[test]
fn scan_all_runs_every_scanner_over_an_empty_home() -> Result<()> {
    let home = tempdir()?;
    let ctx = test_ctx(home.path(), vec![home.path().join("none")]);
    let items = scan_all(&ctx);
    assert!(
        items
            .windows(2)
            .all(|pair| pair[0].category <= pair[1].category)
    );
    Ok(())
}

#[test]
fn only_filter_still_lets_unselected_categories_raise_the_risk_of_a_parent() -> Result<()> {
    let home = tempdir()?;
    let cache = home.path().join(".cache/huggingface");
    write_file(
        &cache.join("hub/models--org--m/blob"),
        &"x".repeat(MODEL_BYTES),
    )?;
    let mut ctx = test_ctx(home.path(), vec![home.path().join("none")]);
    ctx.config.extra_cache_dirs = vec![cache];
    let items = scan_only(&ctx, &[Category::PackageCaches]);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].risk, Risk::Review);
    assert!(!items[0].preselected);
    Ok(())
}
