# 发版

## GitHub Actions（推荐）

1. 有本项目改动时，在 `CHANGELOG.md` 新增 `Unreleased` 章节记录，提交并推送到 `main`；发版后自动移除该章节，不保留空标题。
2. 打开仓库 **Actions → Release → Run workflow**，选择 `main`，输入新的项目版本号，例如 `0.1.1`。
3. 可先勾选 `dry_run` 演练。演练会实际拉取、验证、构建和打包，产物在该次运行的 Artifacts 中；不会提交、推送 tag 或发布 Release。
4. 正式发版不勾选 `dry_run`。验证成功后自动提交版本和依赖锁定文件、创建 tag、发布 Release，附 `.alfredworkflow` 和 `SHA256SUMS`。

需要仓库允许 Actions 使用 `contents: write`；若 main 保护规则禁止机器人推送，使用下面的本地方式准备版本并走仓库的合并流程，不要关闭保护规则。

项目当前是私有仓库，Release 也继承仓库可见性。

## CodexBar 版本和 Changelog

只有准备发版时访问上游 `releases/latest`，只接受稳定的 `vX.Y.Z`，且必须包含官方 macOS arm64、x86_64 CLI 及 GitHub 提供的 SHA256 digest。缺少摘要或架构、下载校验失败、上游许可证变化均停止发版。

发版准备先验证两份下载，再更新 `vendor/manifest.json`、对应 tag 的许可证、项目版本和文档引用。普通 `python3 scripts/package.py` 只使用锁定 manifest，不自行升级。

`CHANGELOG.md` 自动记录 CodexBar 旧版本 → 新版本、上游发布说明及完整 compare 链接（跨多版本升级也可追溯）。如果上游版本未变，只记录内置版本，不生成虚假的升级条目。上游发布说明是外部链接，不会将未经审核的全部上游改动宣称为本工作流新功能。

项目版本以 `Cargo.toml` 为基准，准备脚本同步 `Cargo.lock` 和 Alfred plist。实际包校验检查 manifest、版本、架构、权限及模拟用量查询。两个架构还会检查 Mach-O 最低系统版本，不满足当前 macOS 14.0+ 承诺就停止发版，需先审查兼容性。CI 仅执行宿主架构的 CodexBar，另一架构检查 Mach-O slice，不假装跨机器端到端测试。

## 本地准备与验证

需要 macOS 14+、Rust stable、Python 3.12+、Xcode Command Line Tools、GitHub CLI、curl、zip。先通过 `gh auth login` 登录，并保持 Git 工作区干净。

```sh
python3 scripts/release.py upstream  # 只读查看最新稳定版及资产摘要
python3 scripts/release.py prepare --version 0.1.1
python3 -m unittest discover -s tests/release -v
cargo test --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo fetch --locked
python3 scripts/package.py
python3 scripts/verify_package.py
```

检查 `git diff` 与 `dist/RELEASE_NOTES.md`。准备命令不会提交或发布；不要对已经准备好的版本重复运行 `prepare`，重建直接运行 `package.py` 即可。

## 中断恢复

- **构建/校验失败**：没有推送版本或 tag。修复问题后重新触发，新的运行会重新解析 latest。
- **main 已前进**：发布推送停止。基于新的 main 重新运行，不强制推送。
- **版本/tag 已存在**：流程拒绝覆盖。已发布版本不可原地替换。
- **tag 已推送但 Release 上传失败**：不要重新运行 `prepare`。下载失败运行的 `release-X.Y.Z` Artifact，检查 `SHA256SUMS`；保留其中 `BASE_COMMIT`、准备补丁和 Release notes 作为证据。若尚未创建 Release，在原 tag 上运行：

```sh
# 在下载 Artifact 的目录中执行，替换下面的示例版本。
shasum -a 256 -c SHA256SUMS
gh release create v0.1.1 alfred-token-peek.alfredworkflow SHA256SUMS \
  --repo chang-open-source/alfred-token-peek-rust --verify-tag \
  --title 'Token Peek v0.1.1' --notes-file RELEASE_NOTES.md --draft
```

若已存在草稿，检查资产并仅补传缺失文件（`gh release upload`，不要使用 `--clobber`）。确认草稿内两个资产齐全且摘要一致后，再执行 `gh release edit v0.1.1 --repo chang-open-source/alfred-token-peek-rust --draft=false`。不要覆盖已公开的 Release。Artifacts 有保留期限，必要时应先保存到本地。
