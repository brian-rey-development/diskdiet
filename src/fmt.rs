use crate::time::SECS_PER_DAY;
use std::path::Path;

pub(crate) const TILDE: &str = "~";

const DAYS_PER_MONTH: u64 = 30;
const DAYS_PER_YEAR: u64 = 365;
const MONTHS_THRESHOLD_DAYS: u64 = 60;
const SIZE_UNIT_FACTORS: [(&str, f64); 5] = [
    ("b", 1.0),
    ("kb", 1e3),
    ("mb", 1e6),
    ("gb", 1e9),
    ("tb", 1e12),
];

pub(crate) fn plural_s(count: u64) -> &'static str {
    if count == 1 { "" } else { "s" }
}

pub(crate) fn count_noun(count: usize, singular: &str, plural: &str) -> String {
    let noun = if count == 1 { singular } else { plural };
    format!("{count} {noun}")
}

pub fn format_age(secs: u64) -> String {
    let days = secs / SECS_PER_DAY;
    match days {
        0 => "today".to_owned(),
        1..MONTHS_THRESHOLD_DAYS => format!("{days} day{} ago", plural_s(days)),
        MONTHS_THRESHOLD_DAYS..DAYS_PER_YEAR => {
            let months = days / DAYS_PER_MONTH;
            format!("{months} months ago")
        }
        _ => {
            let years = days / DAYS_PER_YEAR;
            format!("{years} year{} ago", plural_s(years))
        }
    }
}

pub fn tilde_path(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => TILDE.to_owned(),
        Ok(rest) => format!("{TILDE}/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

pub fn parse_size(text: &str) -> Option<u64> {
    let token = text.split_whitespace().next()?;
    let split = token.find(|c: char| !(c.is_ascii_digit() || c == '.'))?;
    let number: f64 = token[..split].parse().ok()?;
    let unit = token[split..].to_ascii_lowercase();
    let (_, factor) = SIZE_UNIT_FACTORS.iter().find(|(name, _)| *name == unit)?;
    whole_bytes(number * factor)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn whole_bytes(value: f64) -> Option<u64> {
    let rounded = value.round();
    (rounded.is_finite() && rounded >= 0.0 && rounded <= u64::MAX as f64).then_some(rounded as u64)
}
