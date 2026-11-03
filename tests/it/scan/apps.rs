use crate::support::{SECS_PER_DAY, STALE_DAYS};
use anyhow::Result;
use diskdiet::scan::apps::{
    Spotlight, app_data_paths, latest_activity, parse_mdls_batch, parse_mdls_date, stale_reason,
    terminal_app_name,
};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

const MDLS_SAMPLE_EPOCH: i64 = 1_778_980_370;
const SAFARI_AND_CALCULATOR: &str =
    "com.apple.Safari\u{0}2026-09-14 13:45:27 +0000\0com.apple.calculator\0(null)";
const SAFARI_EPOCH: i64 = 1_789_393_527;

#[test]
fn mdls_dates_parse_to_unix_seconds() {
    assert_eq!(
        parse_mdls_date("2026-05-17 01:12:50 +0000"),
        Some(MDLS_SAMPLE_EPOCH)
    );
    assert_eq!(parse_mdls_date("1970-01-01 00:00:00 +0000"), Some(0));
}

#[test]
fn mdls_dates_apply_the_timezone_offset() {
    assert_eq!(
        parse_mdls_date("2026-05-17 03:12:50 +0200"),
        Some(MDLS_SAMPLE_EPOCH)
    );
    assert_eq!(
        parse_mdls_date("2026-05-16 20:12:50 -0500"),
        Some(MDLS_SAMPLE_EPOCH)
    );
}

#[test]
fn mdls_null_and_garbage_are_none() {
    assert_eq!(parse_mdls_date("(null)"), None);
    assert_eq!(parse_mdls_date(""), None);
    assert_eq!(parse_mdls_date("2026-13-40 99:99:99 +0000"), None);
}

#[test]
fn mdls_dates_ignore_trailing_fields_and_reject_short_ones() {
    assert_eq!(
        parse_mdls_date("2026-09-14-99 13:45:27:00 +0000"),
        Some(SAFARI_EPOCH)
    );
    assert_eq!(parse_mdls_date("2026-09 13:45:27 +0000"), None);
    assert_eq!(parse_mdls_date("2026-09-14 13:45 +0000"), None);
}

#[test]
fn stale_reason_flags_old_and_never_opened_apps() -> Result<()> {
    let now = MDLS_SAMPLE_EPOCH;
    let day = i64::try_from(SECS_PER_DAY)?;
    let old = now - 143 * day;
    let recent = now - 10 * day;
    assert_eq!(
        stale_reason(Some(old), now, STALE_DAYS).as_deref(),
        Some("last opened 143 days ago")
    );
    assert_eq!(stale_reason(Some(recent), now, STALE_DAYS), None);
    assert_eq!(
        stale_reason(None, now, STALE_DAYS).as_deref(),
        Some("no recorded use (Spotlight or app data)")
    );
    Ok(())
}

#[test]
fn terminal_programs_map_to_app_bundles() {
    assert_eq!(
        terminal_app_name("Apple_Terminal").as_deref(),
        Some("Terminal.app")
    );
    assert_eq!(terminal_app_name("iTerm.app").as_deref(), Some("iTerm.app"));
    assert_eq!(terminal_app_name("unknown"), None);
}

#[test]
fn app_data_paths_cover_preferences_containers_and_support() {
    let home = Path::new("/Users/me");
    let paths = app_data_paths(home, Some("com.todoist.mac.Todoist"), "Todoist");
    let shown: Vec<String> = paths
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    assert!(
        shown.contains(&"/Users/me/Library/Preferences/com.todoist.mac.Todoist.plist".to_string())
    );
    assert!(shown.contains(&"/Users/me/Library/Containers/com.todoist.mac.Todoist".to_string()));
    assert!(shown.contains(&"/Users/me/Library/Application Support/Todoist".to_string()));
    assert_eq!(app_data_paths(home, None, "Todoist").len(), 1);
}

#[test]
fn latest_activity_picks_the_newest_existing_path() -> Result<()> {
    let dir = tempdir()?;
    let file = dir.path().join("prefs.plist");
    fs::write(&file, "x")?;
    let missing = dir.path().join("missing");
    assert!(latest_activity(&[missing.clone(), file]).is_some());
    assert_eq!(latest_activity(&[missing]), None);
    Ok(())
}

#[test]
fn batched_mdls_output_yields_one_record_per_app() {
    let records = parse_mdls_batch(SAFARI_AND_CALCULATOR, 2);
    let safari = Spotlight {
        bundle_id: Some("com.apple.Safari".to_owned()),
        last_used: Some(SAFARI_EPOCH),
    };
    let calculator = Spotlight {
        bundle_id: Some("com.apple.calculator".to_owned()),
        last_used: None,
    };
    assert_eq!(records, Some(vec![safari, calculator]));
}

#[test]
fn batched_mdls_output_with_a_missing_app_is_rejected() {
    assert_eq!(parse_mdls_batch(SAFARI_AND_CALCULATOR, 3), None);
    assert_eq!(parse_mdls_batch(SAFARI_AND_CALCULATOR, 1), None);
    assert_eq!(parse_mdls_batch("", 1), None);
}

#[test]
fn batched_mdls_output_with_a_trailing_separator_is_rejected() {
    let padded = format!("{SAFARI_AND_CALCULATOR}\0");
    assert_eq!(parse_mdls_batch(&padded, 2), None);
}

#[test]
fn batched_mdls_null_fields_become_none() {
    let records = parse_mdls_batch("(null)\0(null)", 1);
    assert_eq!(records, Some(vec![Spotlight::default()]));
}

#[test]
fn batched_mdls_fields_are_trimmed_before_parsing() {
    let records = parse_mdls_batch("com.example.app\n\0 2026-09-14 13:45:27 +0000 \n", 1);
    let expected = Spotlight {
        bundle_id: Some("com.example.app".to_owned()),
        last_used: Some(SAFARI_EPOCH),
    };
    assert_eq!(records, Some(vec![expected]));
}
