use diskdiet::app::ui::share::{boxed, percent_encode, tweet_url, tweet_url_for_label};
use diskdiet::infra::sys::platform::MACHINE;

#[test]
fn percent_encode_keeps_unreserved_and_escapes_the_rest() {
    assert_eq!(percent_encode("AZaz09-_.~"), "AZaz09-_.~");
    assert_eq!(percent_encode("a b\n&/:"), "a%20b%0A%26%2F%3A");
    assert_eq!(percent_encode("é"), "%C3%A9");
}

#[test]
fn tweet_url_for_label_is_a_single_encoded_intent_link() {
    let url = tweet_url_for_label("287 GB");
    let machine = percent_encode(MACHINE);
    let start = format!(
        "https://x.com/intent/post?text=I%20just%20freed%20287%20GB%20on%20my%20{machine}%20with"
    );
    assert!(url.starts_with(&start), "{url}");
    assert!(url.contains("%0A%0Ahttps%3A%2F%2Fgithub.com%2Fbrian-rey-development%2Fdiskdiet"));
    assert!(!url.contains(' ') && !url.contains('\n'));
}

#[test]
fn tweet_url_formats_the_freed_bytes() {
    let url = tweet_url(1_500_000_000);
    assert!(url.contains("1.5%20GB"), "{url}");
}

#[test]
fn boxed_card_has_equal_width_lines() {
    let lines = vec!["short".to_string(), "a much longer line".to_string()];
    let card = boxed(&lines);
    let widths: Vec<usize> = card.lines().map(|line| line.chars().count()).collect();
    assert!(
        widths.windows(2).all(|pair| pair[0] == pair[1]),
        "{widths:?}"
    );
}
