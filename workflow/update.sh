#!/bin/bash
set -euo pipefail

notify() {
  osascript -e 'on run argv' \
    -e 'display notification (item 1 of argv) with title "Token Peek"' \
    -e 'end run' "$1" >/dev/null 2>&1 || true
}

# Run a temporary copy so replacing this workflow cannot interrupt the installer.
work_dir=$(mktemp -d "${TMPDIR:-/tmp}/token-peek-update.XXXXXX")
trap 'rm -rf "$work_dir"' EXIT
cp "$(dirname "$0")/install.sh" "$work_dir/install.sh"
notify '正在检查并更新 Token Peek…'
if (cd "$work_dir" && bash ./install.sh --update) >"$work_dir/result.log" 2>&1; then
  notify "$(tail -n 1 "$work_dir/result.log")"
else
  cat "$work_dir/result.log" >&2
  notify '更新失败，请检查网络或查看 Alfred 调试日志。'
  exit 1
fi
