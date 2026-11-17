#!/bin/bash
set -euo pipefail

repo=chang-open-source/alfred-token-peek-rust
asset=alfred-token-peek.alfredworkflow

fail() { printf '%s\n' "$*" >&2; exit 1; }
[[ "$(uname -s)" == Darwin ]] || fail 'Requires macOS 14+ and Alfred 5 Powerpack.'
version=$(sw_vers -productVersion)
[[ "${version%%.*}" -ge 14 ]] || fail 'Requires macOS 14+.'
tag=$(curl -fsSL "https://api.github.com/repos/$repo/releases/latest" | plutil -extract tag_name raw -o - -)
[[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail 'No stable release found.'
temp_dir=$(mktemp -d "${TMPDIR:-/tmp}/token-peek.XXXXXX")
keep=0
cleanup() { if [[ "$keep" == 0 ]]; then rm -rf "$temp_dir"; fi; }
trap cleanup EXIT

printf 'Downloading Token Peek %s…\n' "$tag"
base="https://github.com/$repo/releases/download/$tag"
for file in "$asset" SHA256SUMS; do
  curl -fL --retry 2 "$base/$file" -o "$temp_dir/$file"
done
expected=$(awk -v name="$asset" '$2 == name { print $1 }' "$temp_dir/SHA256SUMS")
[[ "$expected" =~ ^[0-9a-fA-F]{64}$ ]] || fail 'Invalid SHA256SUMS.'
actual=$(shasum -a 256 "$temp_dir/$asset")
[[ "${actual%% *}" == "$expected" ]] || fail 'Checksum mismatch; installation stopped.'

open "$temp_dir/$asset"
keep=1
printf 'Confirm the import in Alfred. Package: %s\n' "$temp_dir/$asset"
