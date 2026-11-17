use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub windows: Vec<Window>,
    pub updated_at: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Window {
    pub label: String,
    pub used: Option<f64>,
    pub resets_at: Option<String>,
}

pub fn parse_usage(text: &str, provider: &str) -> Result<Snapshot, String> {
    let value: Value = serde_json::from_str(text).map_err(|_| "CLI 返回了无效 JSON")?;
    let rows = match &value {
        Value::Array(rows) => rows.as_slice(),
        Value::Object(_) => std::slice::from_ref(&value),
        _ => return Err("CLI 返回结构不受支持".into()),
    };
    let row = rows
        .iter()
        .find(|r| r["provider"] == provider)
        .ok_or("CLI 未返回此服务")?;
    let usage = row
        .get("usage")
        .filter(|v| v.is_object())
        .ok_or("无法获取用量，请检查服务登录状态或 CodexBar 配置")?;
    let mut windows = Vec::new();
    for (key, fallback) in [
        ("primary", "当前周期"),
        ("secondary", "第二周期"),
        ("tertiary", "第三周期"),
    ] {
        let Some(w) = usage.get(key).filter(|v| v.is_object()) else {
            continue;
        };
        let label = match w["windowMinutes"].as_u64() {
            Some(300) => "5 小时".into(),
            Some(10080) => "每周".into(),
            Some(m) if m > 0 && m % 1440 == 0 => format!("{} 天", m / 1440),
            Some(m) if m > 0 && m % 60 == 0 => format!("{} 小时", m / 60),
            Some(m) if m > 0 => format!("{m} 分钟"),
            _ => row["rateWindowLabels"][key]
                .as_str()
                .unwrap_or(fallback)
                .to_string(),
        };
        let label = row["rateWindowLabels"][key]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .map(str::to_owned)
            .unwrap_or(label);
        windows.push(Window {
            label,
            used: w["usedPercent"]
                .as_f64()
                .filter(|n| n.is_finite() && (0.0..=100.0).contains(n)),
            resets_at: w["resetsAt"].as_str().map(str::to_owned),
        });
    }
    if windows.is_empty() {
        return Err("此服务未返回套餐额度窗口".into());
    }
    Ok(Snapshot {
        windows,
        updated_at: usage["updatedAt"].as_str().map(str::to_owned),
    })
}
