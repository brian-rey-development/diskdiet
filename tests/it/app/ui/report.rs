use crate::support::fixtures::fixture_item;
use anyhow::Result;
use diskdiet::app::ui::report::report_json;
use diskdiet::domain::model::{Action, Category, Item, Risk};
use serde_json::{Map, Value};

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
fn report_json_lists_every_item_with_its_risk() -> Result<()> {
    let mut risky = fixture_item(Action::EmptyTrash);
    risky.risk = Risk::Danger;
    let text = report_json(&[fixture_item(Action::EmptyTrash), risky])?;
    let value: Value = serde_json::from_str(&text)?;
    assert_eq!(value.as_array().map(Vec::len), Some(2));
    assert_eq!(value[0]["risk"], "safe");
    assert_eq!(value[1]["risk"], "danger");
    assert_eq!(value[0]["bytes"], 1024);
    Ok(())
}

#[test]
fn report_json_pins_every_key_in_order() -> Result<()> {
    let text = report_json(&[sample_item(Risk::Safe)])?;
    assert_eq!(text, GOLDEN_SAFE_ITEM);
    Ok(())
}

#[test]
fn report_json_emits_exactly_six_keys_alphabetically() -> Result<()> {
    let text = report_json(&[sample_item(Risk::Danger)])?;
    let rows: Vec<Map<String, Value>> = serde_json::from_str(&text)?;
    let keys: Vec<&str> = rows[0].keys().map(String::as_str).collect();
    assert_eq!(keys, GOLDEN_KEY_ORDER);
    assert_eq!(rows[0]["risk"], "danger");
    assert_eq!(rows[0]["preselected"], false);
    Ok(())
}

#[test]
fn report_json_of_nothing_is_an_empty_array() -> Result<()> {
    assert_eq!(report_json(&[])?, "[]");
    Ok(())
}
