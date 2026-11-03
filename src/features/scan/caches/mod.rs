mod app;
mod owners;
mod package;

use super::ScanContext;
use crate::domain::model::Item;

pub use app::{app_cache_risk, app_cache_risk_now};
pub(crate) use owners::owner_check;
pub use owners::{OwnerCheck, listing_has_process, named_owner_in, owner_check_in, path_owner_in};

pub fn scan(ctx: &ScanContext) -> Vec<Item> {
    let processes = owners::running_processes();
    let (mut items, app_items) = rayon::join(
        || package::package_caches(ctx, &processes),
        || app::app_caches(ctx, &processes),
    );
    items.extend(app_items);
    items
}
