use token_peek::{
    model::parse_usage,
    render::render,
    runtime::{now, Cache, Entry},
};
#[test]
fn shows_remaining_filters_and_marks_stale() {
    let mut c = Cache::default();
    c.providers.insert(
        "codex".into(),
        Entry {
            snapshot: Some(parse_usage(include_str!("fixtures/usage.json"), "codex").unwrap()),
            fetched_at: now().saturating_sub(400),
            ..Default::default()
        },
    );
    let v = render(&c, "codex", false);
    let text = v.to_string();
    assert!(text.contains("72%"));
    assert!(text.contains("过期"));
    assert!(!v["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["arg"] == "claude"));
    assert!(text.contains("refresh"));
}
#[test]
fn loading_reruns_but_error_stops_reruns() {
    let c = Cache::default();
    assert_eq!(render(&c, "", true)["rerun"], 1);
    assert!(render(&c, "", false).get("rerun").is_none());
}
#[test]
fn unknown_search_does_not_show_unrelated_providers() {
    let v = render(&Cache::default(), "xyz", false);
    assert!(v.to_string().contains("没有匹配"));
}
#[test]
fn old_measurement_is_not_presented_as_fresh() {
    let mut c = Cache::default();
    c.providers.insert("codex".into(),Entry{snapshot:Some(parse_usage(r#"{"provider":"codex","usage":{"primary":{"usedPercent":10},"updatedAt":"2020-01-01T00:00:00Z"}}"#,"codex").unwrap()),fetched_at:now(),..Default::default()});
    let value = render(&c, "codex", false).to_string();
    assert!(value.contains("来源数据已过期"));
    assert!(value.contains("2020-01-01"));
}
#[test]
fn disabled_claude_hides_even_cached_errors() {
    let mut c = Cache::default();
    c.providers.insert(
        "claude".into(),
        Entry {
            error: Some("Claude timeout".into()),
            ..Default::default()
        },
    );
    let v = token_peek::render::render_selected(&c, "", false, &["codex"]);
    assert!(!v.to_string().contains("Claude"));
}
#[test]
fn loading_and_refresh_actions_are_scoped_per_provider() {
    let v =
        token_peek::render::render_status(&Cache::default(), "", &["claude"], &["codex", "claude"]);
    let rows = v["items"].as_array().unwrap();
    let codex = rows.iter().find(|r| r["arg"] == "codex").unwrap();
    let claude = rows.iter().find(|r| r["arg"] == "claude").unwrap();
    assert!(!codex["title"].as_str().unwrap().contains("正在获取"));
    assert!(claude["title"].as_str().unwrap().contains("正在获取"));
    assert_eq!(codex["mods"]["cmd"]["arg"], "refresh:codex");
    assert_eq!(codex["mods"]["cmd"]["valid"], true);
    let filtered = token_peek::render::render_status(
        &Cache::default(),
        "codex",
        &["claude"],
        &["codex", "claude"],
    );
    assert!(filtered.get("rerun").is_none());
    assert!(filtered["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["arg"] == "refresh:codex"));
}
