use anyhow::Context;
use std::path::Path;

pub fn free_bytes(path: &Path) -> anyhow::Result<u64> {
    let stat = rustix::fs::statvfs(path)
        .with_context(|| format!("statvfs failed for {}", path.display()))?;
    Ok(stat.f_bavail.saturating_mul(stat.f_frsize))
}
