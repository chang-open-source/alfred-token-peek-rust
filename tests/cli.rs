use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
#[test]
fn invalid_cache_path_reports_error_instead_of_infinite_rerun() {
    let path = std::env::temp_dir().join(format!(
        "token-peek-invalid-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::write(&path, "not a directory").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_token-peek"))
        .arg("filter")
        .env("CODEXBAR_BIN", "/usr/bin/true")
        .env("alfred_workflow_cache", &path)
        .output()
        .unwrap();
    let _ = fs::remove_file(path);
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value.get("rerun").is_none());
    assert!(!output.status.success());
}
#[test]
fn failed_worker_is_not_restarted_in_the_same_alfred_session() {
    let path = std::env::temp_dir().join(format!(
        "token-peek-writefail-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(path.join("usage.json")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_token-peek"))
        .arg("filter")
        .env("CODEXBAR_BIN", "/usr/bin/true")
        .env("alfred_workflow_cache", &path)
        .env("token_peek_started_codex", "1")
        .env("CLAUDE_ENABLED", "false")
        .output()
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value.get("rerun").is_none());
    assert!(value.to_string().contains("缓存"));
    let _ = fs::remove_dir_all(path);
}
#[test]
fn successful_cache_clears_session_refresh_guard() {
    let path = std::env::temp_dir().join(format!("token-peek-clear-{}", std::process::id()));
    fs::create_dir_all(&path).unwrap();
    let mut cache = token_peek::runtime::Cache::default();
    for p in ["codex", "claude"] {
        cache.providers.insert(
            p.into(),
            token_peek::runtime::Entry {
                attempted_at: token_peek::runtime::now(),
                ..Default::default()
            },
        );
    }
    fs::write(path.join("usage.json"), serde_json::to_vec(&cache).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_token-peek"))
        .arg("filter")
        .env("CODEXBAR_BIN", "/usr/bin/true")
        .env("alfred_workflow_cache", &path)
        .env("token_peek_started_codex", "1")
        .env("CLAUDE_ENABLED", "false")
        .output()
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    fs::remove_dir_all(path).unwrap();
    assert_eq!(value["variables"]["token_peek_started_codex"], "0");
}
#[test]
fn explicit_disabled_claude_never_shows_cached_error() {
    let path = std::env::temp_dir().join(format!("token-peek-disable-{}", std::process::id()));
    fs::create_dir_all(&path).unwrap();
    let mut cache = token_peek::runtime::Cache::default();
    cache.providers.insert(
        "codex".into(),
        token_peek::runtime::Entry {
            attempted_at: token_peek::runtime::now(),
            ..Default::default()
        },
    );
    cache.providers.insert(
        "claude".into(),
        token_peek::runtime::Entry {
            error: Some("Claude timeout".into()),
            ..Default::default()
        },
    );
    fs::write(path.join("usage.json"), serde_json::to_vec(&cache).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_token-peek"))
        .arg("filter")
        .env("CLAUDE_ENABLED", "false")
        .env("CODEXBAR_BIN", "/usr/bin/true")
        .env("alfred_workflow_cache", &path)
        .output()
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    fs::remove_dir_all(path).unwrap();
    assert!(!value.to_string().contains("Claude"));
    assert!(value.get("rerun").is_none());
}
#[test]
fn filtered_queries_and_manual_refresh_only_start_requested_provider() {
    use std::{
        os::unix::fs::PermissionsExt,
        time::{Duration, Instant},
    };
    let root = std::env::temp_dir().join(format!("token-peek-scoped-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let fake = root.join("codexbar");
    fs::write(&fake,format!("#!/bin/sh\necho \"$3\" >> '{}'\nsleep 1\necho '[{{\"provider\":\"codex\",\"usage\":{{\"primary\":{{\"usedPercent\":25}}}}}}]'\n",root.join("calls").display())).unwrap();
    fs::write(root.join("claude"), "#!/bin/sh\nexit 0\n").unwrap();
    for p in [&fake, &root.join("claude")] {
        fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let invoke = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_token-peek"))
            .args(args)
            .env("CODEXBAR_BIN", &fake)
            .env("CLAUDE_ENABLED", "auto")
            .env("PATH", format!("{}:/usr/bin:/bin", root.display()))
            .env("alfred_workflow_cache", root.join("cache"))
            .output()
            .unwrap()
    };
    let output = invoke(&["filter", "codex"]);
    assert!(output.status.success());
    let deadline = Instant::now() + Duration::from_secs(4);
    while !root.join("cache/codex/usage.json").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(fs::read_to_string(root.join("calls")).unwrap(), "codex\n");
    let start = Instant::now();
    assert!(invoke(&["start-refresh", "codex"]).status.success());
    assert!(start.elapsed() < Duration::from_millis(800));
    let deadline = Instant::now() + Duration::from_secs(4);
    while token_peek::runtime::is_provider_refreshing(&root.join("cache"), "codex").unwrap()
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        fs::read_to_string(root.join("calls")).unwrap(),
        "codex\ncodex\n"
    );
    assert!(!root.join("cache/claude").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn update_command_does_not_require_cli_or_usage_cache() {
    let output = Command::new(env!("CARGO_BIN_EXE_token-peek"))
        .args(["filter", " UPDATE "])
        .env("CODEXBAR_BIN", "/missing/codexbar")
        .env("alfred_workflow_cache", "/dev/null/cache")
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["items"].as_array().unwrap().len(), 1);
    assert_eq!(value["items"][0]["arg"], "update");
    assert_eq!(value["items"][0]["valid"], true);
    assert!(value.get("rerun").is_none());
}

#[test]
fn alfred_6_offers_native_details_only_for_quota_windows() {
    use serde_json::json;
    let root = std::env::temp_dir().join(format!("token-peek-details-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let timestamp = token_peek::runtime::now();
    let cache = json!({"providers":{"codex":{
        "fetched_at":timestamp - 400, "attempted_at":timestamp,
        "snapshot":{"windows":[
            {"label":"Session", "used":28}, {"label":"Weekly", "used":100},
            {"label":"Other", "used":null}
        ]}
    }}});
    fs::write(root.join("usage.json"), cache.to_string()).unwrap();
    for (version, target) in [("6.0.3", "6"), ("6.0.3", "5"), ("5.8.2", "5"), ("", "6")] {
        let output = Command::new(env!("CARGO_BIN_EXE_token-peek"))
            .args(["filter", "codex"])
            .env("alfred_version", version)
            .env("token_peek_alfred_major", target)
            .env("alfred_workflow_cache", &root)
            .env("CODEXBAR_BIN", "/usr/bin/true")
            .output()
            .unwrap();
        assert!(output.status.success());
        let view: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let rows = view["items"].as_array().unwrap();
        assert_eq!(rows[0]["arg"], "codex");
        assert_eq!(rows[0]["mods"]["cmd"]["arg"], "refresh:codex");
        if version == "6.0.3" && target == "6" {
            let details = &rows[0]["mods"]["alt"];
            assert_eq!(details["valid"], true);
            assert_eq!(
                details["variables"]["quota_summary"],
                rows[0]["text"]["copy"]
            );
            assert!(rows[0]["subtitle"].as_str().unwrap().contains("⌥↩"));
            assert_eq!(details["variables"]["quota_value"], "剩余 72%");
            assert_eq!(details["variables"]["quota_fraction"], "0.72");
            assert!(details["variables"]["quota_status"]
                .as_str()
                .unwrap()
                .contains("缓存已过期"));
            assert_eq!(
                rows[1]["mods"]["alt"]["variables"]["quota_hide_bar"],
                "false"
            );
            assert_eq!(rows[1]["mods"]["alt"]["variables"]["quota_fraction"], "0");
            assert_eq!(
                rows[2]["mods"]["alt"]["variables"]["quota_hide_bar"],
                "true"
            );
            assert_eq!(
                rows[2]["mods"]["alt"]["variables"]["quota_value"],
                "用量未知"
            );
            assert_eq!(rows.last().unwrap()["mods"]["alt"]["valid"], false);
        } else {
            assert!(rows[0]["mods"].get("alt").is_none());
        }
    }
    fs::remove_dir_all(root).unwrap();
}
