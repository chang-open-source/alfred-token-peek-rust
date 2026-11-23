use serde_json::json;
use std::{
    env,
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};
use token_peek::{discovery::available_providers, render::render_status_with_details, runtime};
fn cache_dir() -> PathBuf {
    env::var_os("alfred_workflow_cache")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("HOME").unwrap_or_else(|| "/tmp".into()))
                .join("Library/Caches/com.chang.token-peek")
        })
}
fn cli_path() -> PathBuf {
    if let Some(path) = env::var_os("CODEXBAR_BIN").filter(|s| !s.is_empty()) {
        return PathBuf::from(path);
    }
    let root = env::current_exe()
        .unwrap_or_default()
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86_64"
    };
    root.join("vendor").join(arch).join("CodexBarCLI")
}
fn spawn_worker(provider: &str, force: bool) -> Result<(), String> {
    Command::new(env::current_exe().map_err(|_| "无法定位工作流程序")?)
        .args([if force { "force-worker" } else { "worker" }, provider])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "无法启动后台刷新")?;
    Ok(())
}
fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let dir = cache_dir();
    let cli = cli_path();
    let timeout = Duration::from_secs(20);
    let providers = available_providers();
    match args.first().map(String::as_str).unwrap_or("filter") {
        "--version" => println!("token-peek {}", env!("CARGO_PKG_VERSION")),
        "--help" => println!(
            "token-peek [filter QUERY | refresh [PROVIDERS] | start-refresh PROVIDERS | --version]"
        ),
        "worker" | "force-worker" | "refresh" | "start-refresh" => {
            let selected: Vec<_> = match args.get(1) {
                Some(names) => {
                    let requested: Vec<_> = names.split(',').collect();
                    if requested.iter().any(|p| !runtime::PROVIDERS.contains(p)) {
                        return Err("未知服务".into());
                    }
                    providers
                        .iter()
                        .copied()
                        .filter(|p| requested.contains(p))
                        .collect()
                }
                None => providers.clone(),
            };
            if args[0] == "start-refresh" {
                for &p in &selected {
                    spawn_worker(p, true)?;
                }
                // Give workers time to acquire their locks before Alfred is reopened.
                for p in selected {
                    for _ in 0..25 {
                        if runtime::is_provider_refreshing(&dir, p)? {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
            } else {
                runtime::refresh_selected(&dir, &cli, args[0] != "worker", timeout, &selected)?;
            }
        }
        "filter" => {
            let query = args.get(1).map(String::as_str).unwrap_or("");
            let native_details = env::var("alfred_version")
                .ok()
                .and_then(|version| version.split('.').next()?.parse::<u32>().ok())
                .is_some_and(|major| major >= 6)
                && env::var("token_peek_alfred_major").as_deref() != Ok("5");
            let normalized = query.trim().to_lowercase();
            if normalized == "update" {
                println!(
                    "{}",
                    json!({"items": [{
                        "uid": "update",
                        "title": "更新 Token Peek",
                        "subtitle": format!("当前 v{} · 回车检查并自动更新，保留已有设置", env!("CARGO_PKG_VERSION")),
                        "arg": "update",
                        "mods": {"alt": {"valid": false}},
                        "valid": true
                    }]})
                );
                return Ok(());
            }
            let selected: Vec<_> = providers
                .iter()
                .copied()
                .filter(|p| normalized.is_empty() || p.contains(&normalized))
                .collect();
            if !cli.is_file() {
                println!(
                    "{}",
                    json!({"items":[{"title":"找不到 CodexBar CLI","subtitle":"重新导入完整工作流，或检查 CODEXBAR_BIN 路径","valid":false}]})
                );
                return Ok(());
            }
            let states = selected
                .iter()
                .map(|&p| runtime::is_provider_refreshing(&dir, p).map(|busy| (p, busy)))
                .collect::<Result<Vec<_>, _>>()?;
            // Read after lock probes so a completed worker's cache cannot be missed.
            let cache = runtime::read_cache(&dir);
            let mut refreshing = Vec::new();
            let mut variables = serde_json::Map::new();
            let mut failures = Vec::new();
            for (p, mut loading) in states {
                let key = format!("token_peek_started_{p}");
                let started = env::var(&key).as_deref() == Ok("1");
                let due = runtime::due(cache.providers.get(p));
                if !loading && due && !started {
                    spawn_worker(p, false)?;
                    loading = true;
                }
                if loading {
                    refreshing.push(p);
                }
                variables.insert(
                    key,
                    json!(if loading || (due && started) {
                        "1"
                    } else {
                        "0"
                    }),
                );
                if !loading && due && started {
                    failures.push(p);
                }
            }
            let mut output =
                render_status_with_details(&cache, query, &refreshing, &providers, native_details);
            output["variables"] = variables.into();
            for p in failures {
                output["items"].as_array_mut().unwrap().insert(0,json!({"title":format!("{p} · 后台刷新未能保存结果"),"subtitle":"请检查缓存目录权限和磁盘空间，关闭 Alfred 后重试或手动刷新","valid":false}));
            }
            if native_details {
                for item in output["items"].as_array_mut().unwrap() {
                    if item["mods"]["alt"].is_null() {
                        item["mods"]["alt"] = json!({"valid": false});
                    }
                }
            }
            println!("{output}");
        }
        _ => return Err("未知命令；使用 --help 查看用法".into()),
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        println!(
            "{}",
            json!({"items":[{"title":"Token Peek 暂时不可用","subtitle":error,"valid":false}]})
        );
        std::process::exit(1);
    }
}
