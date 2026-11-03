use diskdiet::model::Risk;
use diskdiet::scan::apps::{
    app_data_paths, latest_activity, parse_mdls_date, staleness, terminal_app_name,
};
use diskdiet::scan::caches::{
    app_cache_risk, app_cache_risk_now, owner_is_running, path_owner_is_running,
};
use diskdiet::scan::docker::{is_local_endpoint, parse_docker_df, parse_docker_size, prune_risk};
use diskdiet::scan::downloads::{classify_download, is_installer};
use diskdiet::scan::models::{hf_label, parse_ollama_list};
use std::path::Path;

const STALE_DAYS: u64 = 90;
const SECS_PER_DAY: i64 = 86_400;
const MDLS_SAMPLE_EPOCH: i64 = 1_778_980_370;

#[test]
fn docker_sizes_parse_with_and_without_percentage() {
    assert_eq!(parse_docker_size("37.23GB (49%)"), Some(37_230_000_000));
    assert_eq!(parse_docker_size("17.74GB"), Some(17_740_000_000));
    assert_eq!(parse_docker_size("512MB"), Some(512_000_000));
    assert_eq!(parse_docker_size("1.5kB"), Some(1_500));
    assert_eq!(parse_docker_size("0B"), Some(0));
}

#[test]
fn docker_sizes_reject_garbage() {
    assert_eq!(parse_docker_size("garbage"), None);
    assert_eq!(parse_docker_size(""), None);
    assert_eq!(parse_docker_size("512"), None);
    assert_eq!(parse_docker_size("12XB"), None);
    assert_eq!(parse_docker_size("N/A"), None);
}

#[test]
fn docker_df_lines_map_type_to_reclaimable_bytes() {
    let text = concat!(
        r#"{"Active":"3","Reclaimable":"37.23GB (49%)","Size":"75GB","TotalCount":"10","Type":"Images"}"#,
        "\n",
        r#"{"Active":"0","Reclaimable":"17.74GB","Size":"17.74GB","TotalCount":"90","Type":"Build Cache"}"#,
        "\n",
        "not json\n",
    );
    let parsed = parse_docker_df(text);
    assert_eq!(
        parsed,
        vec![
            ("Images".to_string(), 37_230_000_000),
            ("Build Cache".to_string(), 17_740_000_000),
        ]
    );
}

#[test]
fn ollama_list_skips_header_and_parses_rows() {
    let text = "NAME            ID              SIZE      MODIFIED\n\
                qwen3:8b        500a1f067a9f    5.2 GB    2 weeks ago\n\
                llama3.2:latest a80c4f17acd5    2.0 GB    3 months ago\n\
                nomic-embed-text:latest 0a109f422b47 274 MB 5 days ago\n";
    assert_eq!(
        parse_ollama_list(text),
        vec![
            (vec!["qwen3:8b".to_string()], 5_200_000_000),
            (vec!["llama3.2:latest".to_string()], 2_000_000_000),
            (vec!["nomic-embed-text:latest".to_string()], 274_000_000),
        ]
    );
}

#[test]
fn ollama_tags_of_the_same_model_are_one_item() {
    let text = "NAME ID SIZE MODIFIED\n\
                llama3.2:latest a80c4f17acd5 2.0 GB 3 months ago\n\
                llama3.2:3b a80c4f17acd5 2.0 GB 3 months ago\n";
    let names = vec!["llama3.2:latest".to_string(), "llama3.2:3b".to_string()];
    assert_eq!(parse_ollama_list(text), vec![(names, 2_000_000_000)]);
}

#[test]
fn docker_images_need_review_and_build_cache_is_safe() {
    assert_eq!(prune_risk("Images"), Some(Risk::Review));
    assert_eq!(prune_risk("Build Cache"), Some(Risk::Safe));
}

#[test]
fn docker_is_only_pruned_on_a_local_socket() {
    assert!(is_local_endpoint(
        "unix:///Users/me/.colima/default/docker.sock\n"
    ));
    assert!(!is_local_endpoint("ssh://deploy@prod.example.com"));
    assert!(!is_local_endpoint("tcp://10.0.0.5:2376"));
}

#[test]
fn derived_data_is_review_while_xcode_runs() {
    let derived = Path::new("/Users/me/Library/Developer/Xcode/DerivedData");
    let xcode = "/Applications/Xcode.app/Contents/MacOS/Xcode\n";
    assert!(path_owner_is_running(derived, xcode));
    assert!(!path_owner_is_running(derived, "/sbin/launchd\n"));
}

#[test]
fn ollama_list_tolerates_empty_and_broken_output() {
    assert!(parse_ollama_list("").is_empty());
    assert!(parse_ollama_list("NAME ID SIZE MODIFIED\n").is_empty());
    assert!(parse_ollama_list("Error: could not connect to ollama app\n").is_empty());
}

#[test]
fn huggingface_folder_names_become_org_slash_name() {
    assert_eq!(
        hf_label("models--mistralai--Mistral-7B-v0.1").as_deref(),
        Some("mistralai/Mistral-7B-v0.1")
    );
    assert_eq!(hf_label("datasets--org--thing"), None);
    assert_eq!(hf_label("models--"), None);
}

#[test]
fn app_cache_risk_follows_the_plan() {
    assert_eq!(app_cache_risk("com.apple.foo"), None);
    assert_eq!(app_cache_risk("CloudKit"), None);
    assert_eq!(app_cache_risk("GeoServices"), None);
    assert_eq!(app_cache_risk("pip"), None);
    assert_eq!(app_cache_risk("ms-playwright"), Some(Risk::Safe));
    assert_eq!(app_cache_risk("notion-updater"), Some(Risk::Safe));
    assert_eq!(app_cache_risk("Google"), Some(Risk::Safe));
    assert_eq!(app_cache_risk("SomeRandomApp"), Some(Risk::Review));
}

const CHROME_RUNNING: &str =
    "/sbin/launchd\n/Applications/Google Chrome.app/Contents/MacOS/Google Chrome\n";

#[test]
fn a_running_app_turns_its_safe_cache_into_review() {
    assert!(owner_is_running("Google", CHROME_RUNNING));
    assert!(!owner_is_running("Firefox", CHROME_RUNNING));
    assert_eq!(
        app_cache_risk_now("Google", CHROME_RUNNING),
        Some(Risk::Review)
    );
    assert_eq!(
        app_cache_risk_now("Firefox", CHROME_RUNNING),
        Some(Risk::Safe)
    );
    assert_eq!(
        app_cache_risk_now("ms-playwright", CHROME_RUNNING),
        Some(Risk::Safe)
    );
    assert_eq!(app_cache_risk_now("com.apple.foo", CHROME_RUNNING), None);
}

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
fn staleness_flags_old_and_never_opened_apps() {
    let now = MDLS_SAMPLE_EPOCH;
    let old = now - 143 * SECS_PER_DAY;
    let recent = now - 10 * SECS_PER_DAY;
    assert_eq!(
        staleness(Some(old), now, STALE_DAYS).as_deref(),
        Some("last opened 143 days ago")
    );
    assert_eq!(staleness(Some(recent), now, STALE_DAYS), None);
    assert_eq!(
        staleness(None, now, STALE_DAYS).as_deref(),
        Some("no recorded use (Spotlight or app data)")
    );
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
fn installers_older_than_a_week_are_safe_and_recent_ones_need_review() {
    assert_eq!(
        classify_download("Docker.dmg", 0, STALE_DAYS),
        Some(Risk::Review)
    );
    assert_eq!(
        classify_download("Docker.dmg", 8, STALE_DAYS),
        Some(Risk::Safe)
    );
    assert_eq!(
        classify_download("tool.PKG", 400, STALE_DAYS),
        Some(Risk::Safe)
    );
    assert_eq!(
        classify_download("ubuntu.iso", 1, STALE_DAYS),
        Some(Risk::Review)
    );
    assert!(is_installer("setup.dmg"));
    assert!(!is_installer("dmg"));
}

#[test]
fn other_downloads_depend_on_age() {
    assert_eq!(
        classify_download("report.pdf", 200, STALE_DAYS),
        Some(Risk::Review)
    );
    assert_eq!(classify_download("report.pdf", 3, STALE_DAYS), None);
    assert_eq!(
        classify_download("report.pdf", STALE_DAYS, STALE_DAYS),
        None
    );
}

#[test]
fn app_data_paths_cover_preferences_containers_and_support() {
    let home = std::path::Path::new("/Users/me");
    let paths = app_data_paths(home, Some("com.todoist.mac.Todoist"), "Todoist");
    let shown: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
    assert!(
        shown.contains(&"/Users/me/Library/Preferences/com.todoist.mac.Todoist.plist".to_string())
    );
    assert!(shown.contains(&"/Users/me/Library/Containers/com.todoist.mac.Todoist".to_string()));
    assert!(shown.contains(&"/Users/me/Library/Application Support/Todoist".to_string()));
    assert_eq!(app_data_paths(home, None, "Todoist").len(), 1);
}

#[test]
fn latest_activity_picks_the_newest_existing_path() -> std::io::Result<()> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("prefs.plist");
    std::fs::write(&file, "x")?;
    let missing = dir.path().join("missing");
    assert!(latest_activity(&[missing.clone(), file]).is_some());
    assert_eq!(latest_activity(&[missing]), None);
    Ok(())
}
