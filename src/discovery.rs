use std::{env, fs, os::unix::fs::PermissionsExt, path::PathBuf};

pub fn has_executable(name: &str, dirs: &[PathBuf]) -> bool {
    dirs.iter().any(|dir| {
        fs::metadata(dir.join(name))
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    })
}

pub fn has_claude(dirs: &[PathBuf], apps: &[PathBuf]) -> bool {
    has_executable("claude", dirs)
        || apps
            .iter()
            .any(|app| has_executable("Claude", &[app.join("Contents/MacOS")]))
}

pub fn available_providers() -> Vec<&'static str> {
    if env::var("CLAUDE_ENABLED")
        .is_ok_and(|v| matches!(v.trim().to_lowercase().as_str(), "false" | "0"))
    {
        return vec!["codex"];
    }
    let mut dirs: Vec<PathBuf> = env::var_os("PATH")
        .map(|p| env::split_paths(&p).filter(|p| p.is_absolute()).collect())
        .unwrap_or_default();
    dirs.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ]);
    let mut apps = vec![PathBuf::from("/Applications/Claude.app")];
    if let Some(home) = env::var_os("HOME").map(PathBuf::from) {
        apps.push(home.join("Applications/Claude.app"));
        for path in [
            ".local/bin",
            ".npm-global/bin",
            ".volta/bin",
            ".claude/local",
        ] {
            dirs.push(home.join(path));
        }
        // Alfred's PATH may omit Node installations managed by nvm.
        if let Ok(versions) = fs::read_dir(home.join(".nvm/versions/node")) {
            dirs.extend(versions.flatten().map(|v| v.path().join("bin")));
        }
    }
    let mut providers = vec!["codex"];
    if has_claude(&dirs, &apps) {
        providers.push("claude");
    }
    providers
}
