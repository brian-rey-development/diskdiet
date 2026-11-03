use diskdiet::app::ui::layout::visible_width;
use diskdiet::app::ui::scan_progress::ScanProgress;
use diskdiet::domain::model::{Action, Category, Item, Risk};
use std::path::PathBuf;

const MEGABYTE: u64 = 1_000_000;
const SCANNERS: usize = 8;

fn item(category: Category, bytes: u64) -> Item {
    let action = Action::DeletePath(PathBuf::from("/tmp/none"));
    Item::new(category, "x", "", bytes, Risk::Safe, action)
}

#[test]
fn the_line_counts_finished_scanners() {
    let mut progress = ScanProgress::new(SCANNERS, &[]);
    progress.record(&[]);
    progress.record(&[]);
    assert_eq!(progress.line(200), "Weighing your disk... 2/8");
}

#[test]
fn categories_are_ranked_by_size_and_summed_across_scanners() {
    let mut progress = ScanProgress::new(SCANNERS, &[]);
    progress.record(&[item(Category::Downloads, 5 * MEGABYTE)]);
    progress.record(&[
        item(Category::Docker, 40 * MEGABYTE),
        item(Category::Downloads, 6 * MEGABYTE),
    ]);
    assert_eq!(
        progress.line(200),
        "Weighing your disk... 2/8 · Docker 40.0 MB · Downloads 11.0 MB"
    );
    assert_eq!(progress.total_bytes(), 51 * MEGABYTE);
}

#[test]
fn categories_outside_only_never_show_up() {
    let mut progress = ScanProgress::new(SCANNERS, &[Category::Downloads]);
    progress.record(&[
        item(Category::Docker, 40 * MEGABYTE),
        item(Category::Downloads, 5 * MEGABYTE),
    ]);
    let line = progress.line(200);
    assert!(!line.contains("Docker"));
    assert!(line.contains("Downloads 5.0 MB"));
    assert_eq!(progress.total_bytes(), 5 * MEGABYTE);
}

#[test]
fn the_line_is_cut_to_the_width_and_keeps_the_biggest_categories_first() {
    let mut progress = ScanProgress::new(SCANNERS, &[]);
    progress.record(&[
        item(Category::Docker, 90 * MEGABYTE),
        item(Category::Downloads, MEGABYTE),
    ]);
    let line = progress.line(45);
    assert_eq!(visible_width(&line), 45);
    assert!(line.ends_with('…'));
    assert!(line.contains("Docker 90.0 MB"));
}
