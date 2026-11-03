use diskdiet::model::Risk;
use diskdiet::scan::docker::{is_local_endpoint, parse_docker_df, prune_risk};

#[test]
fn docker_df_lines_map_type_to_reclaimable_bytes() {
    let text = concat!(
        r#"{"Active":"3","Reclaimable":"37.23GB (49%)","Size":"75GB","TotalCount":"10","Type":"Images"}"#,
        "\n",
        r#"{"Active":"0","Reclaimable":"17.74GB","Size":"17.74GB","TotalCount":"90","Type":"Build Cache"}"#,
        "\n",
        "not json\n",
    );
    let parsed = parse_docker_df(text);
    assert_eq!(
        parsed,
        vec![
            ("Images".to_string(), 37_230_000_000),
            ("Build Cache".to_string(), 17_740_000_000),
        ]
    );
}

#[test]
fn docker_images_need_review_and_build_cache_is_safe() {
    assert_eq!(prune_risk("Images"), Some(Risk::Review));
    assert_eq!(prune_risk("Build Cache"), Some(Risk::Safe));
}

#[test]
fn docker_is_only_pruned_on_a_local_socket() {
    assert!(is_local_endpoint(
        "unix:///Users/me/.colima/default/docker.sock\n"
    ));
    assert!(!is_local_endpoint("ssh://deploy@prod.example.com"));
    assert!(!is_local_endpoint("tcp://10.0.0.5:2376"));
}

#[test]
fn docker_prune_risk_ignores_unknown_kinds() {
    assert_eq!(prune_risk("Containers"), None);
    assert_eq!(prune_risk("Local Volumes"), None);
    assert_eq!(prune_risk("images"), None);
}

#[test]
fn docker_df_lines_with_unparseable_sizes_are_dropped() {
    let text = concat!(
        r#"{"Reclaimable":"N/A","Type":"Images"}"#,
        "\n",
        r#"{"Reclaimable":"0B","Type":"Build Cache"}"#,
        "\n",
    );
    assert_eq!(parse_docker_df(text), vec![("Build Cache".to_owned(), 0)]);
}
