use super::ScanContext;
use super::caches::{make_item, measured};
use crate::model::{Action, Category, Item, Risk};

const TRASH_DIR: &str = ".Trash";
const TRASH_LABEL: &str = "Empty the Trash";
const TRASH_DETAIL: &str = "Permanently deletes everything in ~/.Trash";

pub fn scan(ctx: &ScanContext) -> Vec<Item> {
    let Some(bytes) = measured(&ctx.home.join(TRASH_DIR)) else {
        return Vec::new();
    };
    vec![make_item(
        Category::Trash,
        TRASH_LABEL.to_string(),
        TRASH_DETAIL.to_string(),
        bytes,
        Risk::Review,
        Action::EmptyTrash,
    )]
}
