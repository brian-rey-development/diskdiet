use anyhow::Context;
use std::path::Path;

pub fn free_bytes(path: &Path) -> anyhow::Result<u64> {
    let stat = rustix::fs::statvfs(path)
        .with_context(|| format!("statvfs failed for {}", path.display()))?;
    Ok(stat.f_bavail.saturating_mul(stat.f_frsize))
}

pub(crate) fn total_bytes(path: &Path) -> Option<u64> {
    let stats = rustix::fs::statvfs(path).ok()?;
    Some(stats.f_blocks.saturating_mul(stats.f_frsize))
}
