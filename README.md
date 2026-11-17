# Alfred Token Peek

English · [简体中文](README.zh.md)

Check **Codex and Claude plan usage and reset times** from Alfred. Built in Rust, with the official **CodexBar CLI 0.84.0** bundled for Apple Silicon and Intel. No separate Rust, Python, or CodexBar installation is required.

## Install

Requires **macOS 14+ and Alfred 5 Powerpack**.

One-command install:

```sh
curl -fsSL https://raw.githubusercontent.com/chang-open-source/alfred-token-peek-rust/main/scripts/install.sh | bash
```

Downloads, verifies, and installs the latest release directly into Alfred, preserving existing workflow settings. Or download `.alfredworkflow` from [Releases](https://github.com/chang-open-source/alfred-token-peek-rust/releases) and double-click it.

Sign in to the relevant service and configure a supported [CodexBar data source](https://github.com/steipete/CodexBar/blob/v0.84.0/docs/configuration.md). Detecting an installed app does not confirm authentication.

## Use

| Action | Result |
| --- | --- |
| `usage update` | Check and install workflow updates |
| `usage` | Show enabled services |
| `usage codex` / `usage claude` | Filter by service |
| Return | Open the service's usage page |
| Select “刷新用量” (Refresh usage) | Refresh the current list |
| ⌘ Return / ⌘C | Refresh one service / copy its usage summary |

When cycle data is available, a hint estimates whether quota will last until reset at the current cycle-average rate. This is an estimate, not a guarantee.

Each service refreshes independently. Results are cached for 5 minutes; failed requests retain the last successful data. Unknown usage is never treated as zero. Codex reset credits appear when reported by the data source. This workflow shows plan percentages, not daily token counts or costs. The Alfred interface is currently in Chinese.

Open **Alfred Preferences → Workflows → Token Peek → Configure Workflow** for settings and help. Claude is detected through Claude Code or the desktop app; it can also be hidden manually. Leave the custom CLI path empty to use the bundled version.

## Develop

Requires Rust stable; packaging also needs macOS, Xcode Command Line Tools, Python 3.12+, curl, and zip. Python is used only for build and release tooling.

```sh
python3 -m unittest discover -s tests/release -v
cargo test --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo fetch --locked
python3 scripts/package.py
python3 scripts/verify_package.py
```

Pushes and pull requests run CI automatically. Manual releases resolve the latest stable CodexBar; ordinary builds use the pinned manifest. See [release instructions](docs/releasing.md), [Changelog](CHANGELOG.md), and [verification notes](docs/verification.md) (Chinese).

[MIT](LICENSE). Bundled [CodexBar](https://github.com/steipete/CodexBar) and Rust dependency licenses are included in the package.
