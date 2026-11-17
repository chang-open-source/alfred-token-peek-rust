# Alfred Token Peek

[English](README.md) · 简体中文

在 Alfred 中查看 **Codex、Claude 套餐额度与重置时间**。使用 Rust 编写，内置官方 **CodexBar CLI 0.84.0**，一个安装包支持 Apple Silicon 和 Intel，无需另装 Rust、Python 或 CodexBar。

## 安装

需要 **macOS 14+、Alfred 5 Powerpack**。

一键安装：

```sh
curl -fsSL https://raw.githubusercontent.com/chang-open-source/alfred-token-peek-rust/main/scripts/install.sh | bash
```

自动下载最新版本、校验 SHA256 并打开 Alfred，确认导入即可。也可从 [Releases](https://github.com/chang-open-source/alfred-token-peek-rust/releases) 下载 `.alfredworkflow` 后双击导入。

请先登录对应服务，并配置可用的 [CodexBar 数据源](https://github.com/steipete/CodexBar/blob/v0.84.0/docs/configuration.md)。检测到已安装应用不代表已登录。

## 使用

| 操作 | 功能 |
| --- | --- |
| `usage` | 查看已启用的服务 |
| `usage codex` / `usage claude` | 按服务筛选 |
| 回车 | 打开服务用量页 |
| 选择「刷新用量」 | 刷新当前列表 |
| ⌘ 回车 / ⌘C | 单独刷新某个服务 / 复制用量摘要 |

各服务独立刷新，缓存有效期 5 分钟；查询失败保留上次成功数据，未知额度不按零计算。仅展示套餐百分比，不统计每日 Token 或费用。Alfred 界面目前为中文。

在 **Alfred 偏好设置 → Workflows → Token Peek → Configure Workflow** 中调整设置和查看帮助。自动检测 Claude Code 或桌面版，也可手动隐藏 Claude；自定义 CLI 路径留空即使用内置版本。

## 开发

需要 Rust stable；打包另需 macOS、Xcode Command Line Tools、Python 3.12+、curl 和 zip。Python 仅用于构建与发版工具。

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

推送和 Pull Request 自动运行 CI。手动发版时拉取 CodexBar 最新稳定版，普通构建使用锁定版本。详见[发版说明](docs/releasing.md)、[更新记录](CHANGELOG.md)和[验证记录](docs/verification.md)。

[MIT](LICENSE)。安装包保留 [CodexBar](https://github.com/steipete/CodexBar) 和 Rust 依赖的许可证。
