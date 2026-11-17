use crate::runtime::{now, Cache, PROVIDERS, TTL};
use chrono::{DateTime, Local, Utc};
use serde_json::{json, Value};

pub fn duration_text(seconds: u64) -> String {
    for (unit, label, next_unit, next_label) in [
        (86_400, "天", 3_600, "小时"),
        (3_600, "小时", 60, "分钟"),
        (60, "分钟", 1, "秒"),
    ] {
        if seconds >= unit {
            let major = seconds / unit;
            let minor = seconds % unit / next_unit;
            return if minor == 0 {
                format!("{major} {label}")
            } else {
                format!("{major} {label} {minor} {next_label}")
            };
        }
    }
    format!("{seconds} 秒")
}

pub fn forecast_text(window: &crate::model::Window, observed_at: i64, now: i64) -> Option<String> {
    // Use the observation time, not the render time: cached usage must not improve its own pace.
    if !(0..TTL as i64).contains(&now.checked_sub(observed_at)?) {
        return None;
    }
    let used = window
        .used
        .filter(|v| v.is_finite() && *v > 0.0 && *v <= 100.0)?;
    let duration = i64::try_from(window.window_minutes?.checked_mul(60)?).ok()?;
    let reset = DateTime::parse_from_rfc3339(window.resets_at.as_deref()?)
        .ok()?
        .timestamp();
    let start = reset.checked_sub(duration)?;
    let elapsed = observed_at.checked_sub(start)?;
    if reset <= now || elapsed < 600 || elapsed >= duration {
        return None;
    }
    if used == 100.0 {
        return Some("额度已用尽，等待重置".into());
    }
    let projected = used * duration as f64 / elapsed as f64;
    if projected <= 100.0 {
        Some("按周期均速：预计够用至重置".into())
    } else {
        let early = (duration as f64 - elapsed as f64 * 100.0 / used).ceil() as u64;
        Some(format!("按周期均速：预计提前 {}耗尽", duration_text(early)))
    }
}

pub fn reset_text(raw: Option<&str>) -> String {
    raw.and_then(|r| DateTime::parse_from_rfc3339(r).ok())
        .map(|date| {
            let minutes = (date.with_timezone(&Utc) - Utc::now()).num_minutes();
            let remaining = if minutes < 0 {
                "已到重置时间".into()
            } else if minutes < 60 {
                format!("{minutes} 分钟后重置")
            } else if minutes < 24 * 60 {
                format!("{} 小时 {} 分钟后重置", minutes / 60, minutes % 60)
            } else {
                format!(
                    "{} 天 {} 小时 {} 分钟后重置",
                    minutes / (24 * 60),
                    minutes / 60 % 24,
                    minutes % 60
                )
            };
            format!(
                "{remaining} · {}",
                date.with_timezone(&Local).format("%m-%d %H:%M")
            )
        })
        .unwrap_or_else(|| "重置时间未知".into())
}
fn compact_time(text: &str) -> String {
    text.replace(' ', "")
}

fn window_label(label: &str) -> &str {
    match label.to_ascii_lowercase().as_str() {
        "weekly" => "每周",
        "monthly" => "每月",
        "session" => "本轮",
        _ => label,
    }
}

pub fn render(cache: &Cache, query: &str, loading: bool) -> Value {
    render_selected(cache, query, loading, &PROVIDERS)
}
pub fn render_selected(cache: &Cache, query: &str, loading: bool, providers: &[&str]) -> Value {
    let refreshing = if loading { providers } else { &[] };
    render_status(cache, query, refreshing, providers)
}
pub fn render_status(cache: &Cache, query: &str, refreshing: &[&str], providers: &[&str]) -> Value {
    let query = query.trim().to_lowercase();
    let shown: Vec<_> = providers
        .iter()
        .copied()
        .filter(|p| query.is_empty() || p.contains(&query))
        .collect();
    let mut items = Vec::new();
    for p in providers
        .iter()
        .copied()
        .filter(|p| query.is_empty() || p.contains(&query))
    {
        let loading = refreshing.contains(&p);
        let name = if p == "codex" { "Codex" } else { "Claude" };
        let entry = cache.providers.get(p);
        if let Some(snapshot) = entry.and_then(|e| e.snapshot.as_ref()) {
            let e = entry.unwrap();
            let age = now().saturating_sub(e.fetched_at);
            let stale = age >= TTL || e.error.is_some();
            let measurement = snapshot
                .updated_at
                .as_deref()
                .and_then(|v| DateTime::parse_from_rfc3339(v).ok());
            let source = measurement
                .map(|date| {
                    let stale = Utc::now().signed_duration_since(date).num_seconds() >= TTL as i64;
                    format!(
                        " · 来源 {}{}",
                        date.with_timezone(&Local).format("%Y-%m-%d %H:%M"),
                        if stale {
                            " · 来源数据已过期"
                        } else {
                            ""
                        }
                    )
                })
                .unwrap_or_else(|| " · 来源更新时间未知".into());
            for (i, w) in snapshot.windows.iter().enumerate() {
                let quota = w
                    .used
                    .map(|used| format!("剩余 {:.0}%", 100.0 - used))
                    .unwrap_or_else(|| "用量未知".into());
                let observed_at = match snapshot.updated_at.as_deref() {
                    Some(raw) => DateTime::parse_from_rfc3339(raw)
                        .ok()
                        .map(|date| date.timestamp()),
                    None => i64::try_from(e.fetched_at).ok(),
                };
                let forecast = if stale {
                    None
                } else {
                    observed_at.and_then(|at| forecast_text(w, at, Utc::now().timestamp()))
                };
                let reset = reset_text(w.resets_at.as_deref());
                let source_stale = measurement.is_some_and(|date| {
                    Utc::now().signed_duration_since(date).num_seconds() >= TTL as i64
                });
                let mut parts = Vec::new();
                if source_stale {
                    parts.push("来源数据已过期".into());
                } else if stale {
                    parts.push("缓存已过期".into());
                }
                if loading {
                    parts.push("刷新中".into());
                }
                if let Some(text) = &forecast {
                    let hint = text
                        .strip_prefix("按周期均速：")
                        .map(|text| {
                            format!(
                                "{}（均速）",
                                compact_time(text).replace("预计够用至重置", "预计够用")
                            )
                        })
                        .unwrap_or_else(|| text.clone());
                    parts.push(hint);
                }
                parts.push(compact_time(reset.split(" · ").next().unwrap_or(&reset)));
                if !stale && !source_stale && !loading {
                    parts.push(if age < 60 {
                        "刚更新".into()
                    } else {
                        format!("{}分钟前更新", age / 60)
                    });
                }
                let subtitle = parts.join(" · ");
                let title = format!("{name} · {} · {quota}", window_label(&w.label));
                let used = w
                    .used
                    .map(|value| format!(" · 已用 {value:.0}%"))
                    .unwrap_or_default();
                let details = format!(
                    "{title}{used}\n{}{} · {}前获取{}{}{}",
                    forecast
                        .map(|text| format!("{text} · "))
                        .unwrap_or_default(),
                    reset,
                    duration_text(age),
                    if stale { " · 缓存已过期" } else { "" },
                    if loading { " · 刷新中" } else { "" },
                    source
                );
                items.push(json!({"uid":format!("{p}-{i}"),"title":title,"subtitle":subtitle,"arg":p,"valid":true,"mods":{"cmd":{"arg":format!("refresh:{p}"),"subtitle":"刷新此服务","valid":!loading}},"icon":{"path":format!("icons/{p}.png")},"text":{"copy":details}}));
            }
            if let Some(credits) = snapshot.reset_credits.as_ref().filter(|_| p == "codex") {
                let expired = credits
                    .next_expires_at
                    .as_deref()
                    .and_then(|raw| DateTime::parse_from_rfc3339(raw).ok())
                    .is_some_and(|date| date <= Utc::now());
                let title = format!(
                    "Codex · {} {} 次",
                    if stale || expired {
                        "上次重置卡剩余"
                    } else {
                        "重置卡剩余"
                    },
                    credits.available
                );
                let subtitle = if expired {
                    "有卡片已到期 · 刷新确认剩余次数".to_string()
                } else if stale {
                    "缓存已过期 · ⌘回车刷新".to_string()
                } else if loading {
                    "刷新中 · 回车查看用量页".to_string()
                } else {
                    "回车查看用量页 · ⌘回车刷新".to_string()
                };
                items.push(json!({"uid":"codex-reset-credits", "title":title,
                    "subtitle":subtitle, "arg":"codex", "valid":true,
                    "mods":{"cmd":{"arg":"refresh:codex","subtitle":"刷新重置卡次数","valid":!loading}},
                    "icon":{"path":"icons/codex.png"}, "text":{"copy":format!("{title}\n{subtitle}")}}));
            }
        } else {
            items.push(json!({"uid":format!("{p}-empty"),"title":format!("{name} · {}",if loading {"正在获取用量…"} else {"暂无用量"}),"subtitle":"回车打开服务用量页；使用下方刷新重试","arg":p,"valid":true,"mods":{"cmd":{"arg":format!("refresh:{p}"),"subtitle":"仅刷新此服务","valid":!loading}},"icon":{"path":format!("icons/{p}.png")}}));
        }
        if let Some(error) = entry.and_then(|e| e.error.as_ref()) {
            items.push(json!({"title":format!("{name} · {error}"),"subtitle":"请在工作流配置窗口查看帮助；旧缓存不代表当前额度","valid":false}));
        }
    }
    if items.is_empty() {
        items.push(json!({"title":"没有匹配的服务","subtitle":"仅显示已启用的服务，留空显示全部","valid":false}));
    }
    let loading = shown.iter().any(|p| refreshing.contains(p));
    let can_refresh = shown.iter().any(|p| !refreshing.contains(p));
    items.push(json!({"uid":"refresh","title":if !can_refresh && loading {"正在刷新…"} else {"刷新用量"},"subtitle":"更新当前列表 · ⌘回车刷新单项 · ⌘C 复制详情","arg":format!("refresh:{}",shown.join(",")),"valid":can_refresh}));
    let mut output = json!({"items":items,"skipknowledge":true});
    if loading {
        output["rerun"] = json!(1);
    }
    output
}
