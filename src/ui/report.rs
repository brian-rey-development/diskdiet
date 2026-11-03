use crate::cli::Args;
use crate::model::{Item, Risk};
use crate::size::format_bytes;
use serde_json::json;
use std::io::{self, Write};

const COL_GAP: &str = "  ";
const HEADERS: [&str; 4] = ["CATEGORY", "ITEM", "SIZE", "RISK"];

pub fn risk_name(risk: Risk) -> &'static str {
    match risk {
        Risk::Safe => "safe",
        Risk::Review => "review",
        Risk::Danger => "danger",
    }
}

pub fn run(args: &Args) -> anyhow::Result<()> {
    let (home, config) = super::load_settings(args)?;
    let (_, items) = super::scan_with_tools(home, config, &args.only);
    let text = if args.json {
        report_json(&items)?
    } else {
        report_table(&items)
    };
    write_ignoring_closed_pipe(&text)
}

pub fn write_ignoring_closed_pipe(text: &str) -> anyhow::Result<()> {
    match writeln!(io::stdout().lock(), "{text}") {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        other => Ok(other?),
    }
}

pub fn report_json(items: &[Item]) -> anyhow::Result<String> {
    let rows: Vec<_> = items
        .iter()
        .map(|item| {
            json!({
                "category": item.category.title(),
                "label": item.label,
                "detail": item.detail,
                "bytes": item.bytes,
                "risk": risk_name(item.risk),
                "preselected": item.preselected,
            })
        })
        .collect();
    Ok(serde_json::to_string_pretty(&rows)?)
}

fn row_cells(item: &Item) -> [String; 4] {
    [
        item.category.title().to_string(),
        item.label.clone(),
        format_bytes(item.bytes),
        risk_name(item.risk).to_string(),
    ]
}

fn column_widths(rows: &[[String; 4]]) -> [usize; 4] {
    let mut widths = HEADERS.map(str::len);
    for row in rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    widths
}

fn format_row(cells: &[String; 4], widths: &[usize; 4]) -> String {
    let [category, label, size, risk] = cells;
    let [w0, w1, w2, _] = widths;
    format!("{category:<w0$}{COL_GAP}{label:<w1$}{COL_GAP}{size:>w2$}{COL_GAP}{risk}")
}

pub fn report_table(items: &[Item]) -> String {
    let rows: Vec<[String; 4]> = items.iter().map(row_cells).collect();
    let widths = column_widths(&rows);
    let header = HEADERS.map(str::to_string);
    let total: u64 = items.iter().map(|item| item.bytes).sum();
    let mut lines = vec![format_row(&header, &widths)];
    lines.extend(rows.iter().map(|row| format_row(row, &widths)));
    lines.push(format!("\nTotal: {}", format_bytes(total)));
    lines.join("\n")
}
