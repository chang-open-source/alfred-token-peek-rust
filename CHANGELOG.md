# Changelog

项目改动及内置 CodexBar 更新均在此记录。

## [Unreleased]

### 首次发布

- 在 Alfred 中查看 Codex、Claude 套餐额度及重置时间，支持按服务筛选。
- 各服务独立缓存和后台刷新；失败保留旧数据，未知额度不按零计算。
- 自动检测 Claude Code 或桌面版，提供工作流配置及内嵌帮助。
- 单个安装包支持 Apple Silicon 和 Intel，无需另装 Rust、Python 或 CodexBar。
- 提供中英文 README、自动测试和可校验的手动发版流程。

### 验证范围

- 自动测试覆盖数据解析、缓存、超时、并发刷新、应用检测和发版脚本。
- Codex 真实查询及 Alfred 本机界面已在开发阶段验证；Claude 真实账号查询尚未确认成功。
- 安装包未做 Developer ID 签名或公证，跨机器安装体验尚未完整验证。
