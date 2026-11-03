use super::entries::{Selection, category_entries, item_label};
use super::layout::{fit, note_row_budget, terminal_width};
use crate::fmt::count_noun;
use crate::model::{Category, Item};
use crate::size::format_bytes;
use console::style;
use std::io;

const ITEM_INDENT: &str = "  ";
const REVIEW_TITLE: &str = "Review your picks";
const TOTAL_LABEL: &str = "Total";
const CHOICE_PROMPT: &str = "Does this look right?";
const CONFIRM_LABEL: &str = "Looks right";
const CONFIRM_HINT: &str = "one last question before anything happens";
const EDIT_LABEL: &str = "Change a category";
const EDIT_HINT: &str = "go back and adjust one list";
const CANCEL_LABEL: &str = "Cancel";
const CANCEL_HINT: &str = "nothing is deleted";
const CATEGORY_PROMPT: &str = "Which category?";
const BACK_LABEL: &str = "Back to review";
const SEPARATOR: &str = " · ";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum ReviewChoice {
    Confirm,
    Edit,
    Cancel,
}

pub struct ReviewGroup<'a> {
    pub category: Category,
    pub entries: Vec<(usize, &'a Item)>,
}

impl ReviewGroup<'_> {
    pub fn bytes(&self) -> u64 {
        self.entries.iter().map(|(_, item)| item.bytes).sum()
    }
}

pub struct CategoryProgress {
    pub category: Category,
    pub picked: usize,
    pub available: usize,
    pub picked_bytes: u64,
}

pub fn review_groups<'a>(items: &'a [Item], selected: &Selection) -> Vec<ReviewGroup<'a>> {
    Category::ALL
        .iter()
        .map(|&category| ReviewGroup {
            category,
            entries: category_entries(items, category)
                .into_iter()
                .filter(|(index, _)| selected.contains(index))
                .collect(),
        })
        .filter(|group| !group.entries.is_empty())
        .collect()
}

pub fn review_totals(groups: &[ReviewGroup<'_>]) -> (usize, u64) {
    let count = groups.iter().map(|group| group.entries.len()).sum();
    let bytes = groups.iter().map(ReviewGroup::bytes).sum();
    (count, bytes)
}

fn group_heading(group: &ReviewGroup<'_>) -> String {
    let count = count_noun(group.entries.len(), "item", "items");
    let figures = format!(
        "{SEPARATOR}{count}{SEPARATOR}{}",
        format_bytes(group.bytes())
    );
    format!(
        "{}{}",
        style(group.category.title()).bold(),
        style(figures).dim()
    )
}

fn group_block(group: &ReviewGroup<'_>, budget: usize) -> String {
    let mut lines = vec![fit(&group_heading(group), budget)];
    for (_, item) in &group.entries {
        lines.push(fit(&format!("{ITEM_INDENT}{}", item_label(item)), budget));
    }
    lines.join("\n")
}

fn total_line(groups: &[ReviewGroup<'_>]) -> String {
    let (count, bytes) = review_totals(groups);
    let figures = format!(
        "{TOTAL_LABEL}{SEPARATOR}{}{SEPARATOR}{}",
        count_noun(count, "item", "items"),
        format_bytes(bytes)
    );
    style(figures).bold().green().to_string()
}

pub fn render_review(groups: &[ReviewGroup<'_>], terminal: usize) -> String {
    let budget = note_row_budget(terminal);
    let mut blocks: Vec<String> = groups
        .iter()
        .map(|group| group_block(group, budget))
        .collect();
    blocks.push(fit(&total_line(groups), budget));
    blocks.join("\n\n")
}

pub(super) fn show_review(items: &[Item], selected: &Selection) -> io::Result<()> {
    let groups = review_groups(items, selected);
    cliclack::note(REVIEW_TITLE, render_review(&groups, terminal_width()))
}

pub fn category_progress(items: &[Item], selected: &Selection) -> Vec<CategoryProgress> {
    Category::ALL
        .iter()
        .map(|&category| {
            let entries = category_entries(items, category);
            let picked = entries.iter().filter(|(index, _)| selected.contains(index));
            CategoryProgress {
                category,
                picked: picked.clone().count(),
                available: entries.len(),
                picked_bytes: picked.map(|(_, item)| item.bytes).sum(),
            }
        })
        .filter(|progress| progress.available > 0)
        .collect()
}

pub fn progress_hint(progress: &CategoryProgress) -> String {
    format!(
        "{} of {} selected{SEPARATOR}{}",
        progress.picked,
        progress.available,
        format_bytes(progress.picked_bytes)
    )
}

pub(super) fn ask_choice() -> io::Result<ReviewChoice> {
    cliclack::select(CHOICE_PROMPT)
        .item(ReviewChoice::Confirm, CONFIRM_LABEL, CONFIRM_HINT)
        .item(ReviewChoice::Edit, EDIT_LABEL, EDIT_HINT)
        .item(ReviewChoice::Cancel, CANCEL_LABEL, CANCEL_HINT)
        .interact()
}

pub(super) fn ask_category(items: &[Item], selected: &Selection) -> io::Result<Option<Category>> {
    let mut prompt = cliclack::select(CATEGORY_PROMPT);
    for progress in category_progress(items, selected) {
        prompt = prompt.item(
            Some(progress.category),
            progress.category.title(),
            progress_hint(&progress),
        );
    }
    prompt.item(None, BACK_LABEL, "").interact()
}
