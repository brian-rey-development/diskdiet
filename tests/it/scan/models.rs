use diskdiet::scan::models::{OllamaModel, hf_label, parse_ollama_list};

fn tags_and_bytes(text: &str) -> Vec<(Vec<String>, u64)> {
    parse_ollama_list(text)
        .into_iter()
        .map(|model| (model.tags, model.bytes))
        .collect()
}

fn model(tags: &[&str], bytes: u64) -> OllamaModel {
    OllamaModel {
        tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
        bytes,
    }
}

#[test]
fn ollama_list_skips_header_and_parses_rows() {
    let text = "NAME            ID              SIZE      MODIFIED\n\
                qwen3:8b        500a1f067a9f    5.2 GB    2 weeks ago\n\
                llama3.2:latest a80c4f17acd5    2.0 GB    3 months ago\n\
                nomic-embed-text:latest 0a109f422b47 274 MB 5 days ago\n";
    assert_eq!(
        tags_and_bytes(text),
        vec![
            (vec!["qwen3:8b".to_string()], 5_200_000_000),
            (vec!["llama3.2:latest".to_string()], 2_000_000_000),
            (vec!["nomic-embed-text:latest".to_string()], 274_000_000),
        ]
    );
}

#[test]
fn ollama_tags_of_the_same_model_are_one_item() {
    let text = "NAME ID SIZE MODIFIED\n\
                llama3.2:latest a80c4f17acd5 2.0 GB 3 months ago\n\
                llama3.2:3b a80c4f17acd5 2.0 GB 3 months ago\n";
    let names = vec!["llama3.2:latest".to_string(), "llama3.2:3b".to_string()];
    assert_eq!(tags_and_bytes(text), vec![(names, 2_000_000_000)]);
}

#[test]
fn ollama_list_tolerates_empty_and_broken_output() {
    assert!(parse_ollama_list("").is_empty());
    assert!(parse_ollama_list("NAME ID SIZE MODIFIED\n").is_empty());
    assert!(parse_ollama_list("Error: could not connect to ollama app\n").is_empty());
}

#[test]
fn huggingface_folder_names_become_org_slash_name() {
    assert_eq!(
        hf_label("models--mistralai--Mistral-7B-v0.1").as_deref(),
        Some("mistralai/Mistral-7B-v0.1")
    );
    assert_eq!(hf_label("datasets--org--thing"), None);
    assert_eq!(hf_label("models--"), None);
}

#[test]
fn ollama_models_keep_their_tags_in_listing_order() {
    let text = "NAME ID SIZE MODIFIED\n\
                a:1 1111 1.0 GB 1 day ago\n\
                b:1 2222 2.0 GB 1 day ago\n\
                a:2 1111 1.0 GB 1 day ago\n";
    let expected = vec![
        model(&["a:1", "a:2"], 1_000_000_000),
        model(&["b:1"], 2_000_000_000),
    ];
    assert_eq!(parse_ollama_list(text), expected);
}

#[test]
fn ollama_rows_with_missing_or_unknown_size_are_skipped() {
    assert!(parse_ollama_list("a:1 1111 1.0\n").is_empty());
    assert!(parse_ollama_list("a:1 1111 big GB 1 day ago\n").is_empty());
    assert!(parse_ollama_list("a:1 1111 1.0 XB 1 day ago\n").is_empty());
}
