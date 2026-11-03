use diskdiet::model::{Action, Category, Item, Risk};
use diskdiet::ui::report::report_json;

const GOLDEN_SAFE_ITEM: &str = r#"[
  {
    "bytes": 2048,
    "category": "Build artifacts",
    "detail": "sample detail",
    "label": "sample label",
    "preselected": true,
    "risk": "safe"
  }
]"#;

const GOLDEN_KEY_ORDER: [&str; 6] = [
    "bytes",
    "category",
    "detail",
    "label",
    "preselected",
    "risk",
];

fn sample_item(risk: Risk) -> Item {
    Item::new(
        Category::BuildArtifacts,
        "sample label",
        "sample detail",
        2048,
        risk,
        Action::EmptyTrash,
    )
}

#[test]
fn report_json_pins_every_key_in_order() -> anyhow::Result<()> {
    let text = report_json(&[sample_item(Risk::Safe)])?;
    assert_eq!(text, GOLDEN_SAFE_ITEM);
    Ok(())
}

#[test]
fn report_json_emits_exactly_six_keys_alphabetically() -> anyhow::Result<()> {
    let text = report_json(&[sample_item(Risk::Danger)])?;
    let rows: Vec<serde_json::Map<String, serde_json::Value>> = serde_json::from_str(&text)?;
    let keys: Vec<&str> = rows[0].keys().map(String::as_str).collect();
    assert_eq!(keys, GOLDEN_KEY_ORDER);
    assert_eq!(rows[0]["risk"], "danger");
    assert_eq!(rows[0]["preselected"], false);
    Ok(())
}

#[test]
fn report_json_of_nothing_is_an_empty_array() -> anyhow::Result<()> {
    assert_eq!(report_json(&[])?, "[]");
    Ok(())
}
