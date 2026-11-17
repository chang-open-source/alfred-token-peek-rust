# Contributor Guide

## Scope

- Runtime: Rust in `src/`, Alfred assets in `workflow/`.
- Build/release tooling: Python standard library in `scripts/`; Python is not an end-user dependency.
- Supported providers: Codex and Claude. Preserve independent caches, refresh locks, and nonblocking UI updates.

## Changes

- Keep changes focused and add meaningful tests for behavioral changes.
- Keep `README.md` (English) and `README.zh.md` aligned and concise. Record user-facing changes under `CHANGELOG.md` → `Unreleased`.
- Never commit credentials, real account responses, downloads, or generated binaries. Use synthetic fixtures.
- Preserve unknown-versus-zero usage and cached-data freshness labels. Do not infer authentication from app detection.

## Validation

```sh
python3 -m unittest discover -s tests/release -v
cargo test --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
```

For packaging changes, also run `python3 scripts/package.py` and `python3 scripts/verify_package.py` on macOS. CI runs these checks on pushes and pull requests. Report mock, package, and real-account verification separately.

## Releases

- `Cargo.toml` defines the project version; keep `Cargo.lock` and `workflow/info.plist` consistent.
- Normal builds use `vendor/manifest.json`. Only explicit release preparation resolves the latest stable CodexBar.
- Preserve SHA256 checks, both architectures, licenses, and the macOS 14+ compatibility check.
- Use `scripts/release.py` and the manual Release workflow; see `docs/releasing.md`. Do not publish a release unless requested.
