# Alfred Token Peek

[English](README.md) · 简体中文

在 Alfred 中查看 **Codex、Claude 套餐额度与重置时间**。使用 Rust 编写，内置官方 **CodexBar CLI 0.84.0**，两种 Alfred 安装包均支持 Apple Silicon 和 Intel，无需另装 Rust、Python 或 CodexBar。

## 安装

需要 **macOS 14+ 及 Alfred 5 或 6 的 Powerpack**。

一键安装：

```sh
curl -fsSL https://raw.githubusercontent.com/chang-open-source/alfred-token-peek-rust/main/scripts/install.sh | bash
```

脚本优先识别正在运行的 Alfred，下载并校验对应的安装包，保留已有设置。若同时安装了 5 和 6 且都未运行，可先打开目标版本，或把上面命令末尾的 `bash` 换成 `bash -s -- --alfred-major 6`（或 `5`）。切换 Alfred 主版本后运行 `usage update`，同一工作流版本也会切换安装包。

手动安装时，从 [Releases](https://github.com/chang-open-source/alfred-token-peek-rust/releases) 下载文件名以 `-alfred5.alfredworkflow`（列表版）或 `-alfred6.alfredworkflow`（原生详情版）结尾的包，双击导入。历史 Release 只有一个通用包。旧版更新器首次升级会安装列表版，Alfred 6 用户再运行一次 `usage update` 即可切换为详情版。

请先登录对应服务，并配置可用的 [CodexBar 数据源](https://github.com/steipete/CodexBar/blob/v0.84.0/docs/configuration.md)。检测到已安装应用不代表已登录。

## 使用

| 操作 | 功能 |
| --- | --- |
| `usage update` | 检查并自动更新工作流 |
| `usage` | 查看已启用的服务 |
| `usage codex` / `usage claude` | 按服务筛选 |
| 在额度行按 ⌥ 回车（Alfred 6） | 查看剩余额度、重置时间和缓存状态。可复制摘要、刷新该服务或打开用量页，Esc 返回列表 |
| 回车 | 打开服务用量页 |
| 选择「刷新用量」 | 刷新当前列表 |
| ⌘ 回车 / ⌘C | 单独刷新某个服务 / 复制完整详情 |

周期信息完整时，按本周期平均用量提示额度是否预计够用至重置，仅供参考。

各服务独立刷新，缓存有效期 5 分钟；查询失败保留上次成功数据，未知额度不按零计算。数据源返回时会显示 Codex 剩余重置卡次数。仅展示套餐百分比，不统计每日 Token 或费用。Alfred 界面目前为中文。

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
