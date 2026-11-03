use crate::infra::fs::size::format_bytes;
use crate::infra::sys::platform::MACHINE;
use console::{measure_text_width, style};
use std::fmt::Write;

const INTENT_BASE: &str = "https://x.com/intent/post?text=";
const REPO_URL: &str = "https://github.com/brian-rey-development/diskdiet";
const CARD_PADDING: usize = 2;
const CARD_TITLE: &str = "diskdiet";
const CARD_TAGLINE: &str = "Put your disk on a diet.";

pub(crate) fn tweet_text(freed_label: &str) -> String {
    format!(
        "I just freed {freed_label} on my {MACHINE} with diskdiet. My AI agents had been hoarding build artifacts.\n\n{REPO_URL}"
    )
}

fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~')
}

pub fn percent_encode(text: &str) -> String {
    text.bytes().fold(String::new(), |mut out, byte| {
        if is_unreserved(byte) {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
        out
    })
}

pub fn tweet_url_for_label(freed_label: &str) -> String {
    format!("{INTENT_BASE}{}", percent_encode(&tweet_text(freed_label)))
}

pub fn tweet_url(freed_bytes: u64) -> String {
    tweet_url_for_label(&format_bytes(freed_bytes))
}

pub fn boxed(lines: &[String]) -> String {
    let inner = lines
        .iter()
        .map(|line| measure_text_width(line))
        .max()
        .unwrap_or(0)
        + CARD_PADDING * 2;
    let rule = "─".repeat(inner);
    let body = lines
        .iter()
        .map(|line| boxed_line(line, inner))
        .collect::<Vec<_>>();
    format!("╭{rule}╮\n{}\n╰{rule}╯", body.join("\n"))
}

fn boxed_line(line: &str, inner: usize) -> String {
    let pad = inner.saturating_sub(measure_text_width(line) + CARD_PADDING);
    format!("│{}{line}{}│", " ".repeat(CARD_PADDING), " ".repeat(pad))
}

pub(crate) fn card_lines(freed: &str, before_free: &str, after_free: &str) -> Vec<String> {
    vec![
        style(CARD_TITLE).bold().to_string(),
        style(CARD_TAGLINE).dim().to_string(),
        String::new(),
        format!("Freed       {}", style(freed).green().bold()),
        format!("Free space  {before_free} -> {after_free}"),
        String::new(),
        style(REPO_URL).dim().to_string(),
    ]
}

pub(crate) fn card(freed: u64, before_free: u64, after_free: u64) -> String {
    boxed(&card_lines(
        &format_bytes(freed),
        &format_bytes(before_free),
        &format_bytes(after_free),
    ))
}
