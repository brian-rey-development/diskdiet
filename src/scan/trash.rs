use super::ScanContext;
use super::caches::measured;
use crate::fmt::tilde_path;
use crate::model::{Action, Category, Item, Risk};
use crate::platform::trash_dir;

const TRASH_LABEL: &str = "Empty the Trash";

pub fn scan(ctx: &ScanContext) -> Vec<Item> {
    let trash = trash_dir(&ctx.home);
    let Some(bytes) = measured(&trash) else {
        return Vec::new();
    };
    let detail = format!(
        "Permanently deletes everything in {}",
        tilde_path(&trash, &ctx.home)
    );
    vec![Item::new(
        Category::Trash,
        TRASH_LABEL,
        detail,
        bytes,
        Risk::Review,
        Action::EmptyTrash,
    )]
}
