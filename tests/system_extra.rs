use diskdiet::model::Risk;
use diskdiet::scan::apps::{Spotlight, parse_mdls_batch, parse_mdls_date};
use diskdiet::scan::docker::{parse_docker_df, prune_risk};
use diskdiet::scan::downloads::{classify_download, is_installer};
use diskdiet::scan::models::{OllamaModel, parse_ollama_list};

const SAFARI_AND_CALCULATOR: &str =
    "com.apple.Safari\u{0}2026-09-14 13:45:27 +0000\0com.apple.calculator\0(null)";
const SAFARI_EPOCH: i64 = 1_789_393_527;
const STALE_DAYS: u64 = 90;

fn model(tags: &[&str], bytes: u64) -> OllamaModel {
    OllamaModel {
        tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
        bytes,
    }
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
fn ollama_models_keep_their_tags_in_listing_order() {
    let text = "NAME ID SIZE MODIFIED\n\
                a:1 1111 1.0 GB 1 day ago\n\
                b:1 2222 2.0 GB 1 day ago\n\
                a:2 1111 1.0 GB 1 day ago\n";
    let expected = vec![
        model(&["a:1", "a:2"], 1_000_000_000),
        model(&["b:1"], 2_000_000_000),
    ];
    assert_eq!(parse_ollama_list(text), expected);
}

#[test]
fn ollama_rows_with_missing_or_unknown_size_are_skipped() {
    assert!(parse_ollama_list("a:1 1111 1.0\n").is_empty());
    assert!(parse_ollama_list("a:1 1111 big GB 1 day ago\n").is_empty());
    assert!(parse_ollama_list("a:1 1111 1.0 XB 1 day ago\n").is_empty());
}

#[test]
fn installer_extensions_match_case_insensitively_on_the_last_dot_only() {
    assert!(is_installer("Setup.DMG"));
    assert!(is_installer("pkg.tar.deb"));
    assert!(is_installer("tool.AppImage"));
    assert!(!is_installer("installer.dmg.zip"));
    assert!(!is_installer("rpm"));
}

#[test]
fn download_risk_depends_on_installer_status_and_age() {
    assert_eq!(
        classify_download("a.iso", 7, STALE_DAYS),
        Some(Risk::Review)
    );
    assert_eq!(classify_download("a.iso", 8, STALE_DAYS), Some(Risk::Safe));
    assert_eq!(
        classify_download("a.txt", 91, STALE_DAYS),
        Some(Risk::Review)
    );
    assert_eq!(classify_download("a.txt", 90, STALE_DAYS), None);
}

#[test]
fn docker_prune_risk_ignores_unknown_kinds() {
    assert_eq!(prune_risk("Containers"), None);
    assert_eq!(prune_risk("Local Volumes"), None);
    assert_eq!(prune_risk("images"), None);
}

#[test]
fn docker_df_lines_with_unparseable_sizes_are_dropped() {
    let text = concat!(
        r#"{"Reclaimable":"N/A","Type":"Images"}"#,
        "\n",
        r#"{"Reclaimable":"0B","Type":"Build Cache"}"#,
        "\n",
    );
    assert_eq!(parse_docker_df(text), vec![("Build Cache".to_owned(), 0)]);
}
