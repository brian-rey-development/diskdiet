use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const SECS_PER_MINUTE: u64 = signed::SECS_PER_MINUTE.unsigned_abs();
pub(crate) const SECS_PER_HOUR: u64 = signed::SECS_PER_HOUR.unsigned_abs();
pub(crate) const SECS_PER_DAY: u64 = signed::SECS_PER_DAY.unsigned_abs();

pub(crate) mod signed {
    pub const SECS_PER_MINUTE: i64 = 60;
    pub const SECS_PER_HOUR: i64 = 3_600;
    pub const SECS_PER_DAY: i64 = 86_400;
}

const YEARS_PER_ERA: i64 = 400;
const DAYS_PER_ERA: i64 = 146_097;
const LAST_DAY_OF_ERA: i64 = DAYS_PER_ERA - 1;
const DAYS_PER_YEAR: i64 = 365;
const DAYS_PER_FOUR_YEARS: i64 = 1_460;
const DAYS_PER_CENTURY: i64 = 36_524;
const DAYS_FROM_MARCH_YEAR_ZERO_TO_UNIX_EPOCH: i64 = 719_468;
const MONTHS_PER_YEAR: i64 = 12;
const JANUARY: i64 = 1;
const FEBRUARY: i64 = 2;
const MARCH: i64 = 3;
const DAYS_PER_FIVE_MONTHS: i64 = 153;
const MONTHS_PER_SPAN: i64 = 5;
const SPAN_ROUNDING: i64 = 2;

const DATE_SEPARATOR: char = '-';
const CLOCK_SEPARATOR: char = ':';
const MAX_MONTH: i64 = 12;
const MAX_DAY: i64 = 31;
const MAX_HOUR: i64 = 23;
const MAX_MINUTE_OR_SECOND: i64 = 59;
const OFFSET_HOUR_DIGITS: usize = 2;
const OFFSET_DIGITS: usize = 4;

pub(crate) fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}

pub(crate) fn secs_since(time: SystemTime) -> u64 {
    SystemTime::now()
        .duration_since(time)
        .map_or(0, |elapsed| elapsed.as_secs())
}

pub(crate) fn age_days(time: SystemTime) -> u64 {
    secs_since(time) / SECS_PER_DAY
}

fn month_index_from_march(month: i64) -> i64 {
    (month + MONTHS_PER_YEAR - MARCH) % MONTHS_PER_YEAR
}

fn month_from_march_index(index: i64) -> i64 {
    (index + MARCH - JANUARY) % MONTHS_PER_YEAR + JANUARY
}

fn days_before_month(month_index: i64) -> i64 {
    (DAYS_PER_FIVE_MONTHS * month_index + SPAN_ROUNDING) / MONTHS_PER_SPAN
}

fn month_index_of_day(day_of_year: i64) -> i64 {
    (MONTHS_PER_SPAN * day_of_year + SPAN_ROUNDING) / DAYS_PER_FIVE_MONTHS
}

pub(crate) fn unix_days_from_date(year: i64, month: i64, day: i64) -> i64 {
    let march_year = if month <= FEBRUARY { year - 1 } else { year };
    let era = march_year.div_euclid(YEARS_PER_ERA);
    let year_of_era = march_year - era * YEARS_PER_ERA;
    let day_of_year = days_before_month(month_index_from_march(month)) + day - 1;
    let day_of_era =
        year_of_era * DAYS_PER_YEAR + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * DAYS_PER_ERA + day_of_era - DAYS_FROM_MARCH_YEAR_ZERO_TO_UNIX_EPOCH
}

pub(crate) fn date_from_unix_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + DAYS_FROM_MARCH_YEAR_ZERO_TO_UNIX_EPOCH;
    let era = shifted.div_euclid(DAYS_PER_ERA);
    let day_of_era = shifted.rem_euclid(DAYS_PER_ERA);
    let leap_corrections = day_of_era / DAYS_PER_FOUR_YEARS - day_of_era / DAYS_PER_CENTURY
        + day_of_era / LAST_DAY_OF_ERA;
    let year_of_era = (day_of_era - leap_corrections) / DAYS_PER_YEAR;
    let days_of_whole_years = DAYS_PER_YEAR * year_of_era + year_of_era / 4 - year_of_era / 100;
    let day_of_year = day_of_era - days_of_whole_years;
    let month_index = month_index_of_day(day_of_year);
    let month = month_from_march_index(month_index);
    let day = day_of_year - days_before_month(month_index) + 1;
    let year = year_of_era + era * YEARS_PER_ERA + i64::from(month <= FEBRUARY);
    (year, month, day)
}

pub fn format_timestamp(unix_secs: u64) -> String {
    let days = i64::try_from(unix_secs / SECS_PER_DAY).unwrap_or_default();
    let (year, month, day) = date_from_unix_days(days);
    let secs_of_day = unix_secs % SECS_PER_DAY;
    let hour = secs_of_day / SECS_PER_HOUR;
    let minute = secs_of_day % SECS_PER_HOUR / SECS_PER_MINUTE;
    let second = secs_of_day % SECS_PER_MINUTE;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

fn parse_triple(text: &str, separator: char) -> Option<[i64; 3]> {
    let mut fields = text.split(separator).map(|field| field.parse::<i64>().ok());
    Some([fields.next()??, fields.next()??, fields.next()??])
}

fn parse_date(text: &str) -> Option<[i64; 3]> {
    let date @ [_, month, day] = parse_triple(text, DATE_SEPARATOR)?;
    let valid = (1..=MAX_MONTH).contains(&month) && (1..=MAX_DAY).contains(&day);
    valid.then_some(date)
}

fn parse_clock(text: &str) -> Option<i64> {
    let [hour, minute, second] = parse_triple(text, CLOCK_SEPARATOR)?;
    let valid = (0..=MAX_HOUR).contains(&hour)
        && (0..=MAX_MINUTE_OR_SECOND).contains(&minute)
        && (0..=MAX_MINUTE_OR_SECOND).contains(&second);
    valid.then_some(hour * signed::SECS_PER_HOUR + minute * signed::SECS_PER_MINUTE + second)
}

fn parse_offset(text: &str) -> Option<i64> {
    let (sign, digits) = match text.strip_prefix('+') {
        Some(digits) => (1, digits),
        None => (-1, text.strip_prefix('-')?),
    };
    let hours = digits.get(..OFFSET_HOUR_DIGITS)?.parse::<i64>().ok()?;
    let minutes = digits
        .get(OFFSET_HOUR_DIGITS..OFFSET_DIGITS)?
        .parse::<i64>()
        .ok()?;
    Some(sign * (hours * signed::SECS_PER_HOUR + minutes * signed::SECS_PER_MINUTE))
}

pub fn parse_timestamp(text: &str) -> Option<i64> {
    let mut parts = text.split_whitespace();
    let [year, month, day] = parse_date(parts.next()?)?;
    let clock = parse_clock(parts.next()?)?;
    let offset = parts.next().map_or(Some(0), parse_offset)?;
    Some(unix_days_from_date(year, month, day) * signed::SECS_PER_DAY + clock - offset)
}
