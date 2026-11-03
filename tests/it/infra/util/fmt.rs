use crate::support::SECS_PER_DAY;
use diskdiet::infra::util::fmt::{format_age, parse_size, tilde_path};
use std::path::Path;

#[test]
fn ages_and_paths_read_naturally() {
    assert_eq!(format_age(0), "today");
    assert_eq!(format_age(SECS_PER_DAY), "1 day ago");
    assert_eq!(format_age(9 * SECS_PER_DAY), "9 days ago");
    assert_eq!(format_age(90 * SECS_PER_DAY), "3 months ago");
    assert_eq!(format_age(800 * SECS_PER_DAY), "2 years ago");
    let home = Path::new("/Users/me");
    assert_eq!(tilde_path(&home.join("dev/app"), home), "~/dev/app");
    assert_eq!(tilde_path(Path::new("/opt/x"), home), "/opt/x");
}

#[test]
fn docker_sizes_parse_with_and_without_percentage() {
    assert_eq!(parse_size("37.23GB (49%)"), Some(37_230_000_000));
    assert_eq!(parse_size("17.74GB"), Some(17_740_000_000));
    assert_eq!(parse_size("512MB"), Some(512_000_000));
    assert_eq!(parse_size("1.5kB"), Some(1_500));
    assert_eq!(parse_size("0B"), Some(0));
}

#[test]
fn docker_sizes_reject_garbage() {
    assert_eq!(parse_size("garbage"), None);
    assert_eq!(parse_size(""), None);
    assert_eq!(parse_size("512"), None);
    assert_eq!(parse_size("12XB"), None);
    assert_eq!(parse_size("N/A"), None);
}
