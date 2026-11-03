use crate::cli::Args;
use crate::model::Item;
use crate::session;
use crate::size::format_bytes;
use serde::Serialize;
use std::io::{self, Write};

const COLUMNS: usize = 4;
const COLUMN_GAP: &str = "  ";
const HEADERS: [&str; COLUMNS] = ["CATEGORY", "ITEM", "SIZE", "RISK"];

type Row = [String; COLUMNS];
type Widths = [usize; COLUMNS];

#[derive(Serialize)]
struct JsonRow<'a> {
    bytes: u64,
    category: &'a str,
    detail: &'a str,
    label: &'a str,
    preselected: bool,
    risk: &'a str,
}

pub(crate) fn run(args: &Args) -> anyhow::Result<()> {
    let (home, config) = session::load_settings(args)?;
    let (_, items) = session::scan_with_tools(home, config, &args.only);
    let text = if args.json {
        report_json(&items)?
    } else {
        report_table(&items)
    };
    write_ignoring_closed_pipe(&text)
}

pub(crate) fn write_ignoring_closed_pipe(text: &str) -> anyhow::Result<()> {
    match writeln!(io::stdout().lock(), "{text}") {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        other => Ok(other?),
    }
}

pub fn report_json(items: &[Item]) -> anyhow::Result<String> {
    let rows: Vec<JsonRow> = items
        .iter()
        .map(|item| JsonRow {
            bytes: item.bytes,
            category: item.category.title(),
            detail: &item.detail,
            label: &item.label,
            preselected: item.preselected,
            risk: item.risk.as_str(),
        })
        .collect();
    Ok(serde_json::to_string_pretty(&rows)?)
}

fn row_cells(item: &Item) -> Row {
    [
        item.category.title().to_owned(),
        item.label.clone(),
        format_bytes(item.bytes),
        item.risk.as_str().to_owned(),
    ]
}

fn column_widths(rows: &[Row]) -> Widths {
    let mut widths = HEADERS.map(str::len);
    for row in rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    widths
}

fn format_row(cells: &Row, widths: &Widths) -> String {
    let [category, label, size, risk] = cells;
    let [category_width, label_width, size_width, _] = widths;
    format!(
        "{category:<category_width$}{COLUMN_GAP}{label:<label_width$}{COLUMN_GAP}{size:>size_width$}{COLUMN_GAP}{risk}"
    )
}

pub(crate) fn report_table(items: &[Item]) -> String {
    let rows: Vec<Row> = items.iter().map(row_cells).collect();
    let widths = column_widths(&rows);
    let header = HEADERS.map(str::to_owned);
    let total: u64 = items.iter().map(|item| item.bytes).sum();
    let mut lines = vec![format_row(&header, &widths)];
    lines.extend(rows.iter().map(|row| format_row(row, &widths)));
    lines.push(format!("\nTotal: {}", format_bytes(total)));
    lines.join("\n")
}
