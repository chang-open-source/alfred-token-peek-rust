#!/bin/bash
set -euo pipefail

repo=chang-open-source/alfred-token-peek-rust
asset=alfred-token-peek.alfredworkflow

fail() { printf '%s\n' "$*" >&2; exit 1; }
[[ "$(uname -s)" == Darwin ]] || fail 'Requires macOS 14+ and Alfred 5 Powerpack.'
version=$(sw_vers -productVersion)
[[ "${version%%.*}" -ge 14 ]] || fail 'Requires macOS 14+.'
preferences=${alfred_preferences:-}
if [[ -z "$preferences" ]]; then
  prefs_file="$HOME/Library/Application Support/Alfred/prefs.json"
  if [[ -f "$prefs_file" ]]; then
    preferences=$(plutil -extract current raw -o - "$prefs_file")
  else
    preferences="$HOME/Library/Application Support/Alfred/Alfred.alfredpreferences"
  fi
fi
workflows="$preferences/workflows"
[[ -d "$workflows" ]] || fail 'Alfred preferences not found. Launch Alfred and activate Powerpack first.'

tag=$(curl -fsSL "https://api.github.com/repos/$repo/releases/latest" | plutil -extract tag_name raw -o - -)
[[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail 'No stable release found.'
temp_dir=$(mktemp -d "${TMPDIR:-/tmp}/token-peek.XXXXXX")
stage=
backup=
destination=
lock=
cleanup() {
  if [[ -n "$backup" && -d "$backup" && ! -e "$destination" ]]; then
    mv "$backup" "$destination" || printf 'Restore the previous workflow from: %s\n' "$backup" >&2
  fi
  [[ -z "$stage" ]] || rm -rf "$stage"
  [[ -z "$lock" ]] || rmdir "$lock"
  rm -rf "$temp_dir"
}
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

ditto -x -k "$temp_dir/$asset" "$temp_dir/unpacked"
package="$temp_dir/unpacked"
bundleid=com.chang.token-peek
[[ "$(plutil -extract bundleid raw -o - "$package/info.plist")" == "$bundleid" ]] || fail 'Unexpected workflow bundle ID.'
[[ "$(plutil -extract version raw -o - "$package/info.plist")" == "${tag#v}" ]] || fail 'Unexpected workflow version.'
[[ -x "$package/token-peek" && -x "$package/action.sh" ]] || fail 'Workflow executables are missing.'

mkdir "$workflows/.token-peek-install.lock" 2>/dev/null || fail 'Another installation is running (.token-peek-install.lock).'
lock="$workflows/.token-peek-install.lock"
for directory in "$workflows"/*; do
  [[ -f "$directory/info.plist" ]] || continue
  id=$(plutil -extract bundleid raw -o - "$directory/info.plist" 2>/dev/null || true)
  if [[ "$id" == "$bundleid" ]]; then
    [[ -z "$destination" ]] || fail 'Multiple Token Peek workflows found. Remove duplicates in Alfred first.'
    destination="$directory"
  fi
done
[[ -n "$destination" ]] || destination="$workflows/user.workflow.$(uuidgen)"
stage=$(mktemp -d "$workflows/.token-peek.XXXXXX")
ditto "$package" "$stage"
if [[ -d "$destination" ]]; then
  [[ ! -f "$destination/prefs.plist" ]] || cp -p "$destination/prefs.plist" "$stage/prefs.plist"
  backup="$workflows/.token-peek-backup.$(uuidgen)"
  mv "$destination" "$backup"
fi
mv "$stage" "$destination"
stage=
[[ -z "$backup" ]] || rm -rf "$backup"
backup=
printf 'Installed Token Peek %s. Type usage in Alfred.\n' "$tag"
