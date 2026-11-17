# Changelog

## [Unreleased]

- 精简列表：主行突出剩余额度，副行显示预测与重置倒计时，复制时保留完整详情。
- 按本周期平均用量提示额度是否预计够用至重置；信息不足或数据过期时不预测。
- 显示 Codex 剩余重置卡次数及最近到期时间（数据源返回时）。
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
