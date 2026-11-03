use diskdiet::time;

#[test]
fn format_timestamp_handles_known_instants() {
    assert_eq!(time::format_timestamp(0), "1970-01-01 00:00:00");
    assert_eq!(time::format_timestamp(951_827_696), "2000-02-29 12:34:56");
    assert_eq!(time::format_timestamp(1_767_225_599), "2025-12-31 23:59:59");
}
