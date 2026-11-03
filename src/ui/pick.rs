use super::entries::{DANGER_TAG, Selection, category_entries, display_label};
use super::layout::{fit_lines, list_row_budget, terminal_width};
use super::review::{self, ReviewChoice};
use crate::fmt::count_noun;
use crate::model::{Category, Item, Risk, RunMode};
use crate::size::format_bytes;
use console::style;
use std::io;

pub use super::entries::item_label;

const MAX_ROWS: usize = 12;
const FILTER_THRESHOLD: usize = 10;
const FILTER_HINT: &str = "type to filter";

pub fn starts_selected(item: &Item) -> bool {
    item.preselected && item.risk != Risk::Danger && item.typed_confirmation.is_none()
}

fn is_filterable(entries: &[(usize, &Item)]) -> bool {
    entries.len() > FILTER_THRESHOLD
}

fn prompt_title(category: Category, entries: &[(usize, &Item)]) -> String {
    let bytes: u64 = entries.iter().map(|(_, item)| item.bytes).sum();
    let title = format!(
        "{} · {}",
        category.title(),
        style(format_bytes(bytes)).bold()
    );
    if is_filterable(entries) {
        return format!("{title} {}", style(format!("· {FILTER_HINT}")).dim());
    }
    title
}

fn build_prompt(
    category: Category,
    entries: &[(usize, &Item)],
    initial: &Selection,
) -> cliclack::MultiSelect<usize> {
    let mut prompt = cliclack::multiselect(prompt_title(category, entries))
        .required(false)
        .max_rows(MAX_ROWS);
    if is_filterable(entries) {
        prompt = prompt.filter_mode();
    }
    for (index, item) in entries {
        prompt = prompt.item(*index, item_label(item), &item.detail);
    }
    let preselected = entries
        .iter()
        .map(|(index, _)| *index)
        .filter(|index| initial.contains(index));
    prompt.initial_values(preselected.collect())
}

fn pick_category(
    category: Category,
    entries: &[(usize, &Item)],
    initial: &Selection,
) -> io::Result<Vec<usize>> {
    let blurb = fit_lines(category.blurb(), list_row_budget(terminal_width()));
    cliclack::log::remark(blurb)?;
    build_prompt(category, entries, initial).interact()
}

fn announce_danger(item: &Item) -> io::Result<()> {
    let heading = format!(
        "{} {}",
        style(DANGER_TAG).red().bold(),
        style(display_label(item)).bold()
    );
    let detail = fit_lines(&item.detail, list_row_budget(terminal_width()));
    cliclack::log::warning(format!("{heading}\n{detail}"))
}

pub(crate) fn confirm_typed(item: &Item) -> io::Result<bool> {
    let Some(word) = &item.typed_confirmation else {
        return Ok(true);
    };
    announce_danger(item)?;
    let prompt = format!(
        "Type {} to remove it, or press Enter to keep it",
        style(word).bold()
    );
    let answer: String = cliclack::input(prompt).required(false).interact()?;
    if answer.trim() == word {
        return Ok(true);
    }
    cliclack::log::info(format!("Kept {}", display_label(item)))?;
    Ok(false)
}

fn confirm_new_dangers(
    items: &[Item],
    already_confirmed: &Selection,
    picked: Vec<usize>,
) -> io::Result<Vec<usize>> {
    let mut confirmed = Vec::with_capacity(picked.len());
    for index in picked {
        if already_confirmed.contains(&index) || confirm_typed(&items[index])? {
            confirmed.push(index);
        }
    }
    Ok(confirmed)
}

fn pick_and_confirm(
    items: &[Item],
    category: Category,
    current: &Selection,
    selected: &mut Selection,
) -> io::Result<()> {
    let entries = category_entries(items, category);
    let picked = pick_category(category, &entries, current)?;
    let kept = confirm_new_dangers(items, current, picked)?;
    selected.retain(|index| items[*index].category != category);
    selected.extend(kept);
    Ok(())
}

fn initial_picks(items: &[Item], category: Category) -> Selection {
    category_entries(items, category)
        .into_iter()
        .filter(|(_, item)| starts_selected(item))
        .map(|(index, _)| index)
        .collect()
}

fn first_pass(items: &[Item]) -> io::Result<Selection> {
    let mut selected = Selection::new();
    for category in Category::ALL {
        if category_entries(items, category).is_empty() {
            continue;
        }
        let initial = initial_picks(items, category);
        pick_and_confirm(items, category, &initial, &mut selected)?;
    }
    Ok(selected)
}

fn edit_category(items: &[Item], selected: &mut Selection) -> io::Result<()> {
    let Some(category) = review::ask_category(items, selected)? else {
        return Ok(());
    };
    let current: Selection = selected
        .iter()
        .copied()
        .filter(|index| items[*index].category == category)
        .collect();
    pick_and_confirm(items, category, &current, selected)
}

fn review_loop(items: &[Item], mut selected: Selection) -> io::Result<Option<Vec<usize>>> {
    while !selected.is_empty() {
        review::show_review(items, &selected)?;
        match review::ask_choice()? {
            ReviewChoice::Confirm => return Ok(Some(selected.into_iter().collect())),
            ReviewChoice::Cancel => return Ok(None),
            ReviewChoice::Edit => edit_category(items, &mut selected)?,
        }
    }
    Ok(Some(Vec::new()))
}

pub(crate) fn choose(items: &[Item]) -> io::Result<Option<Vec<usize>>> {
    review_loop(items, first_pass(items)?)
}

pub fn confirm_prompt(count: usize, bytes: u64, mode: RunMode) -> String {
    if mode == RunMode::DryRun {
        return "Dry run: nothing will be deleted. Continue?".to_owned();
    }
    format!(
        "Delete {} ({})?",
        count_noun(count, "item", "items"),
        style(format_bytes(bytes)).bold()
    )
}

pub(crate) fn confirm_all(selected: &[&Item], mode: RunMode) -> io::Result<bool> {
    let bytes = selected.iter().map(|item| item.bytes).sum();
    cliclack::confirm(confirm_prompt(selected.len(), bytes, mode))
        .initial_value(false)
        .interact()
}
