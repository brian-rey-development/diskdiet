use console::style;
use diskdiet::ui::layout::{
    fit, fit_lines, list_row_budget, note_row_budget, progress_message_budget, spinner_row_budget,
    visible_width,
};

#[test]
fn short_text_is_left_alone() {
    assert_eq!(fit("hello", 5), "hello");
    assert_eq!(fit("hello", 80), "hello");
}

#[test]
fn long_text_is_cut_to_the_width_with_an_ellipsis() {
    let cut = fit("abcdefghij", 6);
    assert_eq!(cut, "abcde…");
    assert_eq!(visible_width(&cut), 6);
}

#[test]
fn styled_text_is_measured_without_its_escape_codes() {
    let styled = style("abcdefghij").red().force_styling(true).to_string();
    let cut = fit(&styled, 4);
    assert_eq!(visible_width(&cut), 4);
    assert_eq!(visible_width(&fit(&styled, 10)), 10);
}

#[test]
fn fit_lines_trims_every_line_on_its_own() {
    assert_eq!(fit_lines("abcdefgh\nab\nabcdefgh", 5), "abcd…\nab\nabcd…");
}

#[test]
fn budgets_leave_room_for_the_frame_and_never_underflow() {
    assert!(list_row_budget(80) < 80);
    assert!(note_row_budget(80) < list_row_budget(80));
    assert!(spinner_row_budget(80) < 80);
    assert!(progress_message_budget(80) < spinner_row_budget(80));
    assert_eq!(list_row_budget(3), 0);
    assert_eq!(note_row_budget(0), 0);
    assert_eq!(progress_message_budget(10), 0);
}
