use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    time::{Duration, Instant},
};
use token_peek::runtime::{fetch, read_cache, refresh};
static NEXT_TEMP: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "token-peek-test-{}-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn script(&self, body: &str) -> PathBuf {
        let p = self.0.join("fake cli");
        fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        p
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
const OUTPUT: &str = r#"echo '[{"provider":"codex","usage":{"primary":{"usedPercent":25}}},{"provider":"claude","usage":{"primary":{"usedPercent":50}}}]'"#;
#[test]
fn accepts_usable_json_on_nonzero_exit_and_space_path() {
    let t = Temp::new();
    let p = t.script(&format!("{OUTPUT}\nexit 1"));
    assert_eq!(
        fetch(&p, "codex", Duration::from_secs(2)).unwrap().windows[0].used,
        Some(25.0)
    );
}
#[test]
fn kills_timeout_without_waiting_for_descendants() {
    let t = Temp::new();
    let p = t.script("sleep 10 &\nwait");
    let start = Instant::now();
    assert!(fetch(&p, "codex", Duration::from_millis(150))
        .unwrap_err()
        .contains("超时"));
    assert!(start.elapsed() < Duration::from_secs(2));
}
#[test]
fn retains_good_snapshot_on_refresh_failure() {
    let t = Temp::new();
    let p = t.script(OUTPUT);
    refresh(&t.0, &p, true, Duration::from_secs(2)).unwrap();
    t.script("echo bad-output\nexit 1");
    refresh(&t.0, &p, true, Duration::from_secs(2)).unwrap();
    let c = read_cache(&t.0);
    assert_eq!(
        c.providers["codex"].snapshot.as_ref().unwrap().windows[0].used,
        Some(25.0)
    );
    assert!(c.providers["codex"].error.is_some());
}
#[test]
fn corrupt_cache_recovers_and_fresh_cache_does_not_refetch() {
    let t = Temp::new();
    fs::write(t.0.join("usage.json"), "{").unwrap();
    let p = t.script(OUTPUT);
    refresh(&t.0, &p, false, Duration::from_secs(2)).unwrap();
    t.script("exit 1");
    refresh(&t.0, &p, false, Duration::from_secs(2)).unwrap();
    assert!(read_cache(&t.0).providers["claude"].error.is_none());
}
#[test]
fn concurrent_refreshes_fetch_each_provider_once() {
    let t = Temp::new();
    let log = t.0.join("calls");
    let p = t.script(&format!(
        "echo call >> '{}'\nsleep 0.2\n{OUTPUT}",
        log.display()
    ));
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| refresh(&t.0, &p, false, Duration::from_secs(2)).unwrap());
        }
    });
    assert_eq!(
        fs::read_to_string(log)
            .unwrap_or_else(|e| panic!("{e}; cache={:?}", read_cache(&t.0)))
            .lines()
            .count(),
        2
    );
}
#[test]
fn only_selected_providers_are_queried() {
    let t = Temp::new();
    let p = t.script(OUTPUT);
    token_peek::runtime::refresh_selected(&t.0, &p, true, Duration::from_secs(2), &["codex"])
        .unwrap();
    let cache = read_cache(&t.0);
    assert!(cache.providers.contains_key("codex"));
    assert!(!cache.providers.contains_key("claude"));
}
#[test]
fn fast_provider_is_published_while_other_is_still_running() {
    let t = Temp::new();
    let p = t.script(&format!(
        "if [ \"$3\" = claude ]; then sleep 2; fi\n{OUTPUT}"
    ));
    std::thread::scope(|s| {
        let worker = s.spawn(|| refresh(&t.0, &p, true, Duration::from_secs(4)).unwrap());
        let deadline = Instant::now() + Duration::from_secs(1);
        while !read_cache(&t.0).providers.contains_key("codex") && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let cache = read_cache(&t.0);
        assert!(
            cache.providers.contains_key("codex"),
            "fast result was held until slow service finished"
        );
        assert!(!cache.providers.contains_key("claude"));
        worker.join().unwrap();
    });
}
#[test]
fn one_provider_lock_does_not_block_another_refresh() {
    let t = Temp::new();
    let p = t.script(&format!(
        "if [ \"$3\" = claude ]; then touch '{}' ; sleep 1; fi\n{OUTPUT}",
        t.0.join("started").display()
    ));
    std::thread::scope(|s| {
        let slow = s.spawn(|| {
            token_peek::runtime::refresh_selected(
                &t.0,
                &p,
                true,
                Duration::from_secs(3),
                &["claude"],
            )
            .unwrap()
        });
        let deadline = Instant::now() + Duration::from_secs(1);
        while !t.0.join("started").exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        token_peek::runtime::refresh_selected(&t.0, &p, true, Duration::from_secs(3), &["codex"])
            .unwrap();
        assert!(read_cache(&t.0).providers.contains_key("codex"));
        slow.join().unwrap();
    });
}
#[test]
fn successful_cache_is_fresh_for_five_minutes_but_failure_retries_soon() {
    use token_peek::runtime::{due, now, Entry};
    let mut e = Entry {
        attempted_at: now() - 120,
        ..Default::default()
    };
    assert!(!due(Some(&e)));
    e.attempted_at = now() - 301;
    assert!(due(Some(&e)));
    e.attempted_at = now() - 31;
    e.error = Some("error".into());
    assert!(due(Some(&e)));
}
