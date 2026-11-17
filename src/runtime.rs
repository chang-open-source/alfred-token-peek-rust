use crate::model::{parse_usage, Snapshot};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{OpenOptionsExt, PermissionsExt},
            process::CommandExt,
        },
    },
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const PROVIDERS: [&str; 2] = ["codex", "claude"];
pub const TTL: u64 = 300;
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Cache {
    pub providers: BTreeMap<String, Entry>,
}
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Entry {
    pub snapshot: Option<Snapshot>,
    pub fetched_at: u64,
    pub attempted_at: u64,
    pub error: Option<String>,
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn due(entry: Option<&Entry>) -> bool {
    entry.is_none_or(|e| {
        now().saturating_sub(e.attempted_at) >= if e.error.is_some() { 30 } else { TTL }
            || e.attempted_at > now()
    })
}
pub fn read_cache(dir: &Path) -> Cache {
    // Read old aggregate cache only for providers not yet migrated.
    let mut cache: Cache = File::open(dir.join("usage.json"))
        .ok()
        .and_then(|f| serde_json::from_reader(f.take(1_048_576)).ok())
        .unwrap_or_default();
    for p in PROVIDERS {
        let path = dir.join(p).join("usage.json");
        if path.exists() {
            cache.providers.remove(p);
            if let Some(entry) = File::open(path)
                .ok()
                .and_then(|f| serde_json::from_reader(f.take(1_048_576)).ok())
            {
                cache.providers.insert(p.into(), entry);
            }
        }
    }
    cache
}
fn prepare(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|_| "无法创建缓存目录")?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
        .map_err(|_| "无法设置缓存权限".into())
}
fn write_cache(dir: &Path, cache: &Entry) -> Result<(), String> {
    let tmp = dir.join(format!("usage.{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|_| "无法写缓存")?;
        serde_json::to_writer(&mut file, cache).map_err(|_| "无法编码缓存")?;
        file.flush().map_err(|_| "无法保存缓存")?;
        fs::rename(&tmp, dir.join("usage.json")).map_err(|_| "无法更新缓存")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}
// flock is released by the kernel on exit, including crashes. Never unlink a lock file.
fn lock(dir: &Path) -> Result<Option<File>, String> {
    prepare(dir)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(dir.join("refresh.lock"))
        .map_err(|_| "无法打开刷新锁")?;
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        Ok(Some(file))
    } else if std::io::Error::last_os_error().kind() == std::io::ErrorKind::WouldBlock {
        Ok(None)
    } else {
        Err("无法获取刷新锁".into())
    }
}
pub fn is_provider_refreshing(dir: &Path, provider: &str) -> Result<bool, String> {
    if !PROVIDERS.contains(&provider) {
        return Err("未知服务".into());
    }
    prepare(dir)?;
    Ok(lock(&dir.join(provider))?.is_none())
}

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        // Every invocation owns a separate process group, including helper children.
        unsafe {
            libc::kill(-(self.0.id() as i32), libc::SIGKILL);
        }
        let _ = self.0.wait();
    }
}
pub fn fetch(cli: &Path, provider: &str, timeout: Duration) -> Result<Snapshot, String> {
    let mut child = Process(
        Command::new(cli)
            .args(["usage", "--provider", provider, "--format", "json"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .map_err(|_| "无法启动 CodexBar CLI，请检查文件权限与 macOS 版本")?,
    );
    let mut stdout = child.0.stdout.take().ok_or("无法读取 CLI 输出")?;
    let fd = stdout.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err("无法配置 CLI 输出".into());
    }
    let start = Instant::now();
    let mut bytes = Vec::new();
    loop {
        if start.elapsed() >= timeout {
            return Err("查询超时，请检查网络或服务登录状态".into());
        }
        let exited = child.0.try_wait().map_err(|_| "无法等待 CLI")?.is_some();
        let mut buf = [0u8; 8192];
        loop {
            match stdout.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    bytes.extend_from_slice(&buf[..n]);
                    if bytes.len() > 1_048_576 {
                        return Err("CLI 输出超过大小限制".into());
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => return Err("无法读取 CLI 输出".into()),
            }
        }
        if exited {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| "CLI 输出不是 UTF-8")?;
    parse_usage(text, provider)
}
pub fn refresh(dir: &Path, cli: &Path, force: bool, timeout: Duration) -> Result<(), String> {
    refresh_selected(dir, cli, force, timeout, &PROVIDERS)
}
pub fn refresh_selected(
    dir: &Path,
    cli: &Path,
    force: bool,
    timeout: Duration,
    providers: &[&str],
) -> Result<(), String> {
    if providers.iter().any(|p| !PROVIDERS.contains(p)) {
        return Err("未知服务".into());
    }
    prepare(dir)?;
    std::thread::scope(|scope| {
        let handles: Vec<_> = providers
            .iter()
            .map(|provider| {
                scope.spawn(move || {
                    let own_dir = dir.join(provider);
                    let Some(_guard) = lock(&own_dir)? else {
                        return Ok(());
                    };
                    let mut entry = read_cache(dir)
                        .providers
                        .remove(*provider)
                        .unwrap_or_default();
                    if !force && !due(Some(&entry)) {
                        return Ok(());
                    }
                    match fetch(cli, provider, timeout) {
                        Ok(snapshot) => {
                            entry.snapshot = Some(snapshot);
                            entry.fetched_at = now();
                            entry.error = None;
                        }
                        Err(error) => entry.error = Some(error),
                    }
                    entry.attempted_at = now();
                    write_cache(&own_dir, &entry)
                })
            })
            .collect();
        let mut error = None;
        for h in handles {
            if let Err(e) = h.join().unwrap_or_else(|_| Err("查询进程异常".into())) {
                error = Some(e);
            }
        }
        error.map_or(Ok(()), Err)
    })
}
