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
                    .map(|used| format!("剩余 {:.0}% · 已用 {:.0}%", 100.0 - used, used))
                    .unwrap_or_else(|| "用量未知".into());
                let subtitle = format!(
                    "{} · {}前获取{}{}{}",
                    reset_text(w.resets_at.as_deref()),
                    duration_text(age),
                    if stale { " · 缓存已过期" } else { "" },
                    if loading { " · 刷新中" } else { "" },
                    source
                );
                let title = format!("{name} · {} · {quota}", w.label);
                items.push(json!({"uid":format!("{p}-{i}"),"title":title,"subtitle":subtitle,"arg":p,"valid":true,"mods":{"cmd":{"arg":format!("refresh:{p}"),"subtitle":"仅刷新此服务","valid":!loading}},"icon":{"path":format!("icons/{p}.png")},"text":{"copy":format!("{title}\n{subtitle}")}}));
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
                let subtitle = format!(
                    "{}前获取{}{} · 回车查看用量页",
                    duration_text(age),
                    if expired {
                        " · 有重置卡已到期，请刷新确认"
                    } else if stale {
                        " · 缓存已过期"
                    } else {
                        ""
                    },
                    if loading { " · 刷新中" } else { "" }
                );
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
    items.push(json!({"uid":"refresh","title":if !can_refresh && loading {"正在刷新…"} else {"刷新用量"},"subtitle":"独立刷新当前服务；⌘回车可单独刷新某一项","arg":format!("refresh:{}",shown.join(",")),"valid":can_refresh}));
    let mut output = json!({"items":items,"skipknowledge":true});
    if loading {
        output["rerun"] = json!(1);
    }
    output
}
