#!/bin/zsh
set -eu
case "${1:-}" in
  update) ./update.sh ;;
  codex) /usr/bin/open 'https://chatgpt.com/codex/settings/usage' ;;
  claude) /usr/bin/open 'https://claude.ai/settings/usage' ;;
  refresh:*)
    ./token-peek start-refresh "${1#refresh:}"
    case "${1#refresh:}" in
      codex) query='usage codex' ;;
      claude) query='usage claude' ;;
      *) query='usage ' ;;
    esac
    /usr/bin/osascript -e 'on run argv' -e 'tell application id "com.runningwithcrayons.Alfred" to search (item 1 of argv)' -e 'end run' "$query"
    ;;
  *) exit 1 ;;
esac
