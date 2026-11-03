mod apps;
mod builds;
mod caches;
mod docker;
mod downloads;
mod models;
mod pipeline;
mod worktrees;

use crate::support::fixtures::test_ctx;
use anyhow::Result;
use diskdiet::scan::scan_all;
use tempfile::tempdir;

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
