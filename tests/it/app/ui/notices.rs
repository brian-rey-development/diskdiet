use diskdiet::app::ui::notices::found_summary;

#[test]
fn found_summary_says_item_for_one_and_items_otherwise() {
    assert_eq!(found_summary(0, 0), "Found 0 B across 0 items");
    assert_eq!(
        found_summary(1_200_000_000, 1),
        "Found 1.2 GB across 1 item"
    );
    assert_eq!(
        found_summary(287_000_000_000, 12),
        "Found 287 GB across 12 items"
    );
}
