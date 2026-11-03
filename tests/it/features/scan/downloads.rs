use crate::support::STALE_DAYS;
use diskdiet::domain::model::Risk;
use diskdiet::features::scan::downloads::{classify_download, is_installer};

#[test]
fn linux_packages_count_as_installers() {
    assert!(is_installer("tool_1.2_amd64.deb"));
    assert!(is_installer("tool.x86_64.rpm"));
    assert!(is_installer("Tool.AppImage"));
    assert_eq!(
        classify_download("tool.deb", 30, STALE_DAYS),
        Some(Risk::Safe)
    );
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
