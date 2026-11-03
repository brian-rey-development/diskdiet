use crate::fmt::count_noun;
use crate::model::{Category, Item};
use crate::size::format_bytes;
use console::style;
use std::cmp::Reverse;
use std::io;

const SIZE_COLUMN: usize = 9;
const COLUMN_GAP: &str = "  ";

struct CategoryRow {
    category: Category,
    bytes: u64,
    count: usize,
}

fn category_rows(items: &[Item]) -> Vec<CategoryRow> {
    let mut rows: Vec<CategoryRow> = Category::ALL
        .iter()
        .map(|&category| {
            let own = items.iter().filter(|item| item.category == category);
            CategoryRow {
                category,
                bytes: own.clone().map(|item| item.bytes).sum(),
                count: own.count(),
            }
        })
        .filter(|row| row.count > 0)
        .collect();
    rows.sort_by_key(|row| Reverse(row.bytes));
    rows
}

fn render_row(row: &CategoryRow, title_width: usize) -> String {
    let size = format!("{:>SIZE_COLUMN$}", format_bytes(row.bytes));
    let title = format!("{:<title_width$}", row.category.title());
    let count = count_noun(row.count, "item", "items");
    format!(
        "{title}{COLUMN_GAP}{}{COLUMN_GAP}{}",
        style(size).bold(),
        style(count).dim()
    )
}

fn render_rows(rows: &[CategoryRow]) -> String {
    let title_width = rows
        .iter()
        .map(|row| row.category.title().len())
        .max()
        .unwrap_or(0);
    let lines: Vec<String> = rows
        .iter()
        .map(|row| render_row(row, title_width))
        .collect();
    lines.join("\n")
}

pub(super) fn show_summary(items: &[Item]) -> io::Result<()> {
    let total: u64 = items.iter().map(|item| item.bytes).sum();
    let title = format!(
        "You can lose up to {}",
        style(format_bytes(total)).bold().green()
    );
    cliclack::note(title, render_rows(&category_rows(items)))
}
