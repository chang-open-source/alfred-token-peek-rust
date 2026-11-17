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

#[test]
fn formats_cache_age_in_readable_units() {
    use token_peek::render::duration_text;
    for (seconds, expected) in [
        (0, "0 秒"),
        (59, "59 秒"),
        (60, "1 分钟"),
        (90, "1 分钟 30 秒"),
        (3_599, "59 分钟 59 秒"),
        (3_600, "1 小时"),
        (3_901, "1 小时 5 分钟"),
        (86_400, "1 天"),
        (90_061, "1 天 1 小时"),
    ] {
        assert_eq!(duration_text(seconds), expected);
    }
    let mut cache = Cache::default();
    cache.providers.insert(
        "codex".into(),
        Entry {
            snapshot: Some(parse_usage(include_str!("fixtures/usage.json"), "codex").unwrap()),
            fetched_at: now().saturating_sub(3_900),
            ..Default::default()
        },
    );
    assert!(render(&cache, "codex", false)["items"][0]["subtitle"]
        .as_str()
        .unwrap()
        .contains("1 小时 5 分钟前获取"));
}

#[test]
fn reset_countdown_splits_days_hours_and_minutes() {
    for (minutes, expected) in [
        (6, "6 分钟后重置"),
        (66, "1 小时 6 分钟后重置"),
        (1440, "1 天 0 小时 0 分钟后重置"),
        (147 * 60 + 6, "6 天 3 小时 6 分钟后重置"),
    ] {
        let reset = chrono::Utc::now() + chrono::Duration::seconds(minutes * 60 + 30);
        assert!(token_peek::render::reset_text(Some(&reset.to_rfc3339())).starts_with(expected));
    }
}

#[test]
fn reset_credit_row_marks_stale_or_expired_counts_as_previous_observations() {
    for (age, expires, previous) in [
        (0, "2099-01-01T00:00:00Z", false),
        (400, "2099-01-01T00:00:00Z", true),
        (0, "2020-01-01T00:00:00Z", true),
    ] {
        let mut cache = Cache::default();
        let input = serde_json::json!({"provider":"codex","usage":{},"resetCredits":{"available":2,"nextExpiresAt":expires}});
        cache.providers.insert(
            "codex".into(),
            Entry {
                snapshot: Some(parse_usage(&input.to_string(), "codex").unwrap()),
                fetched_at: now().saturating_sub(age),
                ..Default::default()
            },
        );
        let view = render(&cache, "codex", false);
        let row = &view["items"][0];
        assert_eq!(row["uid"], "codex-reset-credits");
        let title = row["title"].as_str().unwrap();
        assert!(title.contains("2 次"));
        assert_eq!(title.contains("上次"), previous);
        assert_eq!(row["arg"], "codex");
    }
}
