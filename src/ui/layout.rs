use console::{Term, measure_text_width, truncate_str};

const ELLIPSIS: &str = "…";
const LIST_ROW_OVERHEAD: usize = 6;
const NOTE_ROW_OVERHEAD: usize = 7;
const SPINNER_ROW_OVERHEAD: usize = 5;
const PROGRESS_ROW_OVERHEAD: usize = 48;
const MIN_USABLE_WIDTH: usize = 20;

pub(super) fn terminal_width() -> usize {
    let (_, columns) = Term::stderr().size();
    usize::from(columns).max(MIN_USABLE_WIDTH)
}

pub fn fit(text: &str, width: usize) -> String {
    truncate_str(text, width, ELLIPSIS).into_owned()
}

pub fn visible_width(text: &str) -> usize {
    measure_text_width(text)
}

pub fn list_row_budget(terminal: usize) -> usize {
    terminal.saturating_sub(LIST_ROW_OVERHEAD)
}

pub fn note_row_budget(terminal: usize) -> usize {
    terminal.saturating_sub(NOTE_ROW_OVERHEAD)
}

pub fn spinner_row_budget(terminal: usize) -> usize {
    terminal.saturating_sub(SPINNER_ROW_OVERHEAD)
}

pub fn progress_message_budget(terminal: usize) -> usize {
    terminal.saturating_sub(PROGRESS_ROW_OVERHEAD)
}

pub fn fit_lines(text: &str, width: usize) -> String {
    let lines: Vec<String> = text.lines().map(|line| fit(line, width)).collect();
    lines.join("\n")
}
