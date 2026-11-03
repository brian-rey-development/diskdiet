use super::ScanContext;
use super::builds::tilde_path;
use super::caches::{make_item, measured};
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
    vec![make_item(
        Category::Trash,
        TRASH_LABEL.to_string(),
        detail,
        bytes,
        Risk::Review,
        Action::EmptyTrash,
    )]
}
