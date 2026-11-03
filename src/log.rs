use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SECS_PER_DAY: u64 = 86_400;
const SECS_PER_HOUR: u64 = 3_600;
const SECS_PER_MINUTE: u64 = 60;
const DAYS_FROM_CIVIL_EPOCH_SHIFT: u64 = 719_468;
const DAYS_PER_ERA: u64 = 146_097;

pub fn log_path(home: &Path) -> PathBuf {
    crate::platform::log_path(home)
}

// Howard Hinnant's civil_from_days, valid for dates on or after 1970-01-01.
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let shifted = days + DAYS_FROM_CIVIL_EPOCH_SHIFT;
    let era = shifted / DAYS_PER_ERA;
    let day_of_era = shifted % DAYS_PER_ERA;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

pub fn format_timestamp(unix_secs: u64) -> String {
    let (year, month, day) = civil_from_days(unix_secs / SECS_PER_DAY);
    let secs_of_day = unix_secs % SECS_PER_DAY;
    let hour = secs_of_day / SECS_PER_HOUR;
    let minute = secs_of_day % SECS_PER_HOUR / SECS_PER_MINUTE;
    let second = secs_of_day % SECS_PER_MINUTE;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}

fn append(path: &Path, line: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let single_line = line.replace(['\n', '\r'], " ");
    writeln!(file, "{}  {single_line}", format_timestamp(now_secs()))
}

pub fn record(home: &Path, line: &str) {
    let _ = append(&log_path(home), line);
}
