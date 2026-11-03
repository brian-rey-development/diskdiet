use crate::model::{Category, Item, Risk};
use crate::size::format_bytes;
use console::style;
use std::io;

const MAX_ROWS: usize = 12;
const SIZE_COLUMN: usize = 9;
const DANGER_TAG: &str = "DANGER";

fn total_bytes(entries: &[(usize, &Item)]) -> u64 {
    entries.iter().map(|(_, item)| item.bytes).sum()
}

pub fn item_label(item: &Item) -> String {
    let size = format!("{:>SIZE_COLUMN$}", format_bytes(item.bytes));
    if item.risk != Risk::Danger {
        return format!("{size}  {}", item.label);
    }
    let tag = style(DANGER_TAG).red().bold();
    format!("{size}  {tag} {}", style(&item.label).red())
}

pub fn starts_selected(item: &Item) -> bool {
    item.preselected && item.risk != Risk::Danger && item.typed_confirmation.is_none()
}

fn build_prompt(category: Category, entries: &[(usize, &Item)]) -> cliclack::MultiSelect<usize> {
    let title = format!(
        "{} · {}",
        category.title(),
        style(format_bytes(total_bytes(entries))).bold()
    );
    let mut prompt = cliclack::multiselect(title)
        .required(false)
        .max_rows(MAX_ROWS);
    if entries.len() > MAX_ROWS {
        prompt = prompt.filter_mode();
    }
    for (index, item) in entries {
        prompt = prompt.item(*index, item_label(item), &item.detail);
    }
    let initial = entries
        .iter()
        .filter(|(_, item)| starts_selected(item))
        .map(|(i, _)| *i);
    prompt.initial_values(initial.collect())
}

pub fn pick_category(category: Category, entries: &[(usize, &Item)]) -> io::Result<Vec<usize>> {
    cliclack::log::remark(category.blurb())?;
    build_prompt(category, entries).interact()
}

pub fn confirm_typed(item: &Item) -> io::Result<bool> {
    let Some(word) = &item.typed_confirmation else {
        return Ok(true);
    };
    let heading = format!(
        "{} {}",
        style(DANGER_TAG).red().bold(),
        style(&item.label).bold()
    );
    let prompt = format!(
        "{heading}\n{}\nType {} to remove it, or press Enter to keep it",
        item.detail,
        style(word).bold()
    );
    let answer: String = cliclack::input(prompt).required(false).interact()?;
    if answer.trim() == word {
        return Ok(true);
    }
    cliclack::log::info(format!("Kept {}", item.label))?;
    Ok(false)
}

fn confirm_dangerous(items: &[Item], picked: Vec<usize>) -> io::Result<Vec<usize>> {
    let mut confirmed = Vec::with_capacity(picked.len());
    for index in picked {
        if confirm_typed(&items[index])? {
            confirmed.push(index);
        }
    }
    Ok(confirmed)
}

pub fn pick_all(items: &[Item]) -> io::Result<Vec<usize>> {
    let mut chosen = Vec::new();
    for category in Category::ALL {
        let entries: Vec<(usize, &Item)> = items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.category == category)
            .collect();
        if entries.is_empty() {
            continue;
        }
        chosen.extend(pick_category(category, &entries)?);
    }
    confirm_dangerous(items, chosen)
}

pub fn confirm_prompt(count: usize, bytes: u64, dry_run: bool) -> String {
    if dry_run {
        return "Dry run: nothing will be deleted. Continue?".to_string();
    }
    let noun = if count == 1 { "item" } else { "items" };
    format!(
        "Delete {count} {noun} ({})?",
        style(format_bytes(bytes)).bold()
    )
}

pub fn confirm_all(selected: &[&Item], dry_run: bool) -> io::Result<bool> {
    let bytes = selected.iter().map(|item| item.bytes).sum();
    cliclack::confirm(confirm_prompt(selected.len(), bytes, dry_run))
        .initial_value(false)
        .interact()
}
