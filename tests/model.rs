use token_peek::model::parse_usage;
#[test]
fn reads_provider_windows_without_persisting_identity() {
    let s = parse_usage(include_str!("fixtures/usage.json"), "codex").unwrap();
    assert_eq!(s.windows.len(), 2);
    assert_eq!(s.windows[0].used, Some(28.0));
    assert_eq!(s.windows[0].label, "5 小时");
    assert!(!serde_json::to_string(&s).unwrap().contains("not-persisted"));
}
#[test]
fn missing_and_invalid_percent_are_unknown() {
    for value in ["null", "-1", "101", "\"42\""] {
        let s = parse_usage(
            &format!(r#"{{"provider":"codex","usage":{{"primary":{{"usedPercent":{value}}}}}}}"#),
            "codex",
        )
        .unwrap();
        assert_eq!(s.windows[0].used, None);
    }
}
#[test]
fn genuine_zero_is_preserved() {
    let s = parse_usage(
        r#"{"provider":"claude","usage":{"primary":{"usedPercent":0}}}"#,
        "claude",
    )
    .unwrap();
    assert_eq!(s.windows[0].used, Some(0.0));
}
#[test]
fn missing_provider_or_error_is_not_success() {
    assert!(parse_usage(include_str!("fixtures/usage.json"), "claude").is_err());
    assert!(parse_usage("{}", "codex").is_err());
    assert!(parse_usage(r#"{"provider":"codex","usage":{}}"#, "codex").is_err());
}
#[test]
fn preserves_provider_specific_window_labels() {
    let s=parse_usage(r#"{"provider":"claude","rateWindowLabels":{"tertiary":"Sonnet"},"usage":{"tertiary":{"usedPercent":10,"windowMinutes":10080}}}"#,"claude").unwrap();
    assert_eq!(s.windows[0].label, "Sonnet");
}
