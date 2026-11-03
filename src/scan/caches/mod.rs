mod app;
mod owners;
mod package;

use super::ScanContext;
use crate::model::Item;

pub use app::{app_cache_risk, app_cache_risk_now};
pub(crate) use owners::owner_is_running_now;
pub use owners::{owner_is_running, path_owner_is_running, process_matches};

pub fn scan(ctx: &ScanContext) -> Vec<Item> {
    let processes = owners::running_processes();
    let (mut items, app_items) = rayon::join(
        || package::package_caches(ctx, &processes),
        || app::app_caches(ctx, &processes),
    );
    items.extend(app_items);
    items
}
