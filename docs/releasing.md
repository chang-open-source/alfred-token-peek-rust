# 发版

1. 有项目改动时，在 `CHANGELOG.md` 的 `Unreleased` 章节记录并推送到 `main`。
2. 打开 **Actions → Release → Run workflow**，选择 `patch`（修订）、`minor`（功能）或 `major`（大版本），自动计算版本号。
3. 勾选 `dry_run` 仅构建验证；取消勾选则自动提交版本、创建 tag 并发布安装包。

发版时拉取 CodexBar 最新稳定版，验证双架构文件及 SHA256，并记录依赖更新。普通构建使用锁定版本。需要 Actions 的 `contents: write` 权限。

## 本地准备

保持工作区干净，并登录 GitHub CLI：

```sh
python3 scripts/release.py prepare --version 0.1.1
python3 scripts/package.py
python3 scripts/verify_package.py
```

准备命令不提交或发布；检查 `git diff` 和 `dist/RELEASE_NOTES.md` 后再继续。

## 失败恢复

- tag 创建前失败：修复后重新运行。
- tag 已创建但发布失败：从运行的 Artifact 下载原始产物，校验 `SHA256SUMS`，在原 tag 上补传并发布草稿；不要重新准备版本或覆盖已发布文件。
