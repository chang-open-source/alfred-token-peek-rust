use std::{fs, os::unix::fs::PermissionsExt};
use token_peek::discovery::has_executable;
#[test]
fn requires_executable_file_not_config_or_directory() {
    let root = std::env::temp_dir().join(format!("token-peek-discovery-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    assert!(!has_executable("claude", std::slice::from_ref(&root)));
    fs::create_dir(root.join("claude")).unwrap();
    assert!(!has_executable("claude", std::slice::from_ref(&root)));
    fs::remove_dir(root.join("claude")).unwrap();
    fs::write(root.join("claude"), "#!/bin/sh\n").unwrap();
    fs::set_permissions(root.join("claude"), fs::Permissions::from_mode(0o600)).unwrap();
    assert!(!has_executable("claude", std::slice::from_ref(&root)));
    fs::set_permissions(root.join("claude"), fs::Permissions::from_mode(0o700)).unwrap();
    assert!(has_executable("claude", std::slice::from_ref(&root)));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn detects_desktop_without_cli_but_rejects_incomplete_app() {
    use token_peek::discovery::has_claude;
    let root = std::env::temp_dir().join(format!("token-peek-desktop-{}", std::process::id()));
    let app = root.join("Claude.app");
    let apps = std::slice::from_ref(&app);
    assert!(!has_claude(&[], apps));
    fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
    assert!(!has_claude(&[], apps));
    let binary = app.join("Contents/MacOS/Claude");
    fs::write(&binary, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(!has_claude(&[], apps));
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(has_claude(&[], apps));
    fs::write(root.join("claude"), "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(root.join("claude"), fs::Permissions::from_mode(0o700)).unwrap();
    assert!(has_claude(std::slice::from_ref(&root), &[]));
    fs::remove_dir_all(root).unwrap();
}
