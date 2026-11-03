use diskdiet::model::Risk;
use diskdiet::scan::caches::{
    app_cache_risk, app_cache_risk_now, listing_has_process, named_owner_in, path_owner_in,
};
use std::path::Path;

const CHROME_RUNNING: &str =
    "/sbin/launchd\n/Applications/Google Chrome.app/Contents/MacOS/Google Chrome\n";

#[test]
fn linux_process_names_match_whole_names_only() {
    let processes = "systemd\n  chrome\n/usr/lib/firefox/firefox\nchrome_crashpad\n";
    assert!(listing_has_process(processes, "chrome"));
    assert!(listing_has_process(processes, "firefox"));
    assert!(!listing_has_process("chrome_crashpad\n", "chrome"));
    assert!(!listing_has_process(processes, "steam"));
}

#[test]
fn derived_data_is_review_while_xcode_runs() {
    let derived = Path::new("/Users/me/Library/Developer/Xcode/DerivedData");
    let xcode = "/Applications/Xcode.app/Contents/MacOS/Xcode\n";
    assert!(path_owner_in(derived, xcode));
    assert!(!path_owner_in(derived, "/sbin/launchd\n"));
}

#[test]
fn app_cache_risk_separates_system_safe_and_unknown_caches() {
    assert_eq!(app_cache_risk("com.apple.foo"), None);
    assert_eq!(app_cache_risk("CloudKit"), None);
    assert_eq!(app_cache_risk("GeoServices"), None);
    assert_eq!(app_cache_risk("pip"), None);
    assert_eq!(app_cache_risk("ms-playwright"), Some(Risk::Safe));
    assert_eq!(app_cache_risk("notion-updater"), Some(Risk::Safe));
    assert_eq!(app_cache_risk("Google"), Some(Risk::Safe));
    assert_eq!(app_cache_risk("SomeRandomApp"), Some(Risk::Review));
}

#[test]
fn a_running_app_turns_its_safe_cache_into_review() {
    assert!(named_owner_in("Google", CHROME_RUNNING));
    assert!(!named_owner_in("Firefox", CHROME_RUNNING));
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
