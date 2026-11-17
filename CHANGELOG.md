# Changelog

## [Unreleased]

- 重置倒计时超过 24 小时后按天、小时、分钟显示。

## [0.2.0] - 2026-11-17

- 发版改为选择 `patch`、`minor` 或 `major`，自动计算版本号。
- 新增 `usage update` 命令，检查并自动安装工作流更新，保留已有设置。
- 缓存时间按秒、分钟、小时或天显示。
- 新增一键安装命令，下载并校验最新 Release 后自动安装到 Alfred，保留已有设置。

### 依赖更新

- 内置 CodexBar 0.84.0。[上游发布说明](https://github.com/steipete/CodexBar/releases/tag/v0.84.0)。

## [0.1.0] - 2026-11-17

- 在 Alfred 中查看 Codex、Claude 用量与重置时间。
- 支持缓存、后台刷新及 Claude Code / 桌面版检测。
- 支持 Apple Silicon 和 Intel。
- 内置 [CodexBar 0.84.0](https://github.com/steipete/CodexBar/releases/tag/v0.84.0)。
