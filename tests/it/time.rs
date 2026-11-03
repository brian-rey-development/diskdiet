use diskdiet::time::{self, parse_timestamp};

const SAMPLE_EPOCH: i64 = 1_778_980_370;
const SAFARI_EPOCH: i64 = 1_789_393_527;

#[test]
fn format_timestamp_handles_known_instants() {
    assert_eq!(time::format_timestamp(0), "1970-01-01 00:00:00");
    assert_eq!(time::format_timestamp(951_827_696), "2000-02-29 12:34:56");
    assert_eq!(time::format_timestamp(1_767_225_599), "2025-12-31 23:59:59");
}

#[test]
fn timestamps_parse_to_unix_seconds() {
    assert_eq!(
        parse_timestamp("2026-05-17 01:12:50 +0000"),
        Some(SAMPLE_EPOCH)
    );
    assert_eq!(parse_timestamp("1970-01-01 00:00:00 +0000"), Some(0));
}

#[test]
fn timestamps_apply_the_timezone_offset() {
    assert_eq!(
        parse_timestamp("2026-05-17 03:12:50 +0200"),
        Some(SAMPLE_EPOCH)
    );
    assert_eq!(
        parse_timestamp("2026-05-16 20:12:50 -0500"),
        Some(SAMPLE_EPOCH)
    );
}

#[test]
fn timestamp_null_and_garbage_are_none() {
    assert_eq!(parse_timestamp("(null)"), None);
    assert_eq!(parse_timestamp(""), None);
    assert_eq!(parse_timestamp("2026-13-40 99:99:99 +0000"), None);
}

#[test]
fn timestamps_ignore_trailing_fields_and_reject_short_ones() {
    assert_eq!(
        parse_timestamp("2026-09-14-99 13:45:27:00 +0000"),
        Some(SAFARI_EPOCH)
    );
    assert_eq!(parse_timestamp("2026-09 13:45:27 +0000"), None);
    assert_eq!(parse_timestamp("2026-09-14 13:45 +0000"), None);
}
