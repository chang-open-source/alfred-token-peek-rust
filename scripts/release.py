#!/usr/bin/env python3
"""Prepare a release from one resolved upstream stable release; never publish here."""
import argparse
import base64
from datetime import datetime
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import subprocess
import tempfile
import tomllib
from zoneinfo import ZoneInfo

ROOT = Path(__file__).resolve().parents[1]
UPSTREAM = 'steipete/CodexBar'


def version_tuple(value):
    if not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', value):
        raise ValueError(f'Expected stable x.y.z version: {value!r}')
    return tuple(map(int, value.split('.')))


def validate_next_version(version, current):
    if version_tuple(version) <= version_tuple(current):
        raise ValueError(f'New version {version} must be greater than {current}')


def api(path):
    return json.loads(subprocess.check_output(['gh', 'api', path], text=True))


def latest_release():
    return api(f'repos/{UPSTREAM}/releases/latest')


def manifest_from_release(release):
    tag = release['tag_name']
    if release.get('draft') or release.get('prerelease') or not tag.startswith('v'):
        raise ValueError('Upstream release must be a stable vX.Y.Z tag')
    version = tag[1:]
    version_tuple(version)
    archives = {}
    for arch in ('arm64', 'x86_64'):
        name = f'CodexBarCLI-v{version}-macos-{arch}.tar.gz'
        matches = [a for a in release['assets'] if a['name'] == name]
        if len(matches) != 1:
            raise ValueError(f'Expected exactly one upstream asset: {name}')
        digest = matches[0].get('digest') or ''
        if not re.fullmatch(r'sha256:[0-9a-f]{64}', digest):
            raise ValueError(f'Missing official SHA256 digest for {name}')
        archives[arch] = {'filename': name, 'sha256': digest[7:]}
    return {'version': version, 'release': f'https://github.com/{UPSTREAM}/releases/tag/{tag}',
            'archives': archives}


def download(url, dest):
    subprocess.run(['curl', '-fL', '--retry', '2', '--max-time', '180', url, '-o', str(dest)], check=True)


def verified_asset(url, dest, digest):
    dest.parent.mkdir(parents=True, exist_ok=True)
    if dest.exists():
        if hashlib.sha256(dest.read_bytes()).hexdigest() != digest:
            raise ValueError(f'SHA256 mismatch: {dest}; remove this cached download and retry')
        return
    fd, name = tempfile.mkstemp(prefix=dest.name+'.', suffix='.partial', dir=dest.parent)
    os.close(fd)
    partial = Path(name)
    try:
        download(url, partial)
        if hashlib.sha256(partial.read_bytes()).hexdigest() != digest:
            raise ValueError(f'SHA256 mismatch: {dest.name}')
        partial.replace(dest)
    finally:
        partial.unlink(missing_ok=True)


def release_notes(changelog, version):
    version_tuple(version)
    match = re.search(r'^## \['+re.escape(version)+r'\][^\n]*\n(.*?)(?=^## |\Z)', changelog, re.M | re.S)
    if not match:
        raise ValueError(f'Changelog has no section for {version}')
    return match[1].strip()+'\n'


def render_changelog(changelog, version, date, old, upstream):
    version_tuple(version)
    if re.search(r'^## \['+re.escape(version)+r'\]', changelog, re.M):
        raise ValueError(f'Changelog already contains {version}')
    match = re.search(r'^## \[Unreleased\]\s*\n(.*?)(?=^## |\Z)', changelog, re.M | re.S)
    if not match:
        raise ValueError('Changelog needs exactly one ## [Unreleased] section')
    manifest = manifest_from_release(upstream)
    new = manifest['version']
    notes = match[1].strip()
    dependency = f'- 内置 CodexBar {new}（锁定官方双架构 CLI）。[上游发布说明]({manifest["release"]})。'
    if old != new:
        dependency = (f'- CodexBar {old} → {new}。[上游发布说明]({manifest["release"]})；'
                      f'[完整更新比较](https://github.com/{UPSTREAM}/compare/v{old}...v{new})。')
    notes = (notes+'\n\n' if notes else '')+'### 依赖更新\n\n'+dependency+'\n'
    section = f'## [Unreleased]\n\n## [{version}] - {date}\n\n{notes}\n'
    return (changelog[:match.start()]+section+changelog[match.end():]).rstrip()+'\n', notes


def prepare(root, version, upstream):
    current = tomllib.loads((root/'Cargo.toml').read_text())['package']['version']
    validate_next_version(version, current)
    old = json.loads((root/'vendor/manifest.json').read_text())
    manifest = manifest_from_release(upstream)
    if version_tuple(manifest['version']) < version_tuple(old['version']):
        raise ValueError('Latest upstream version would downgrade the pinned CLI')
    date = datetime.now(ZoneInfo('Asia/Shanghai')).date().isoformat()
    changelog, notes = render_changelog((root/'CHANGELOG.md').read_text(), version, date, old['version'], upstream)
    # Finish all remote reads and checks before changing tracked files.
    for spec in manifest['archives'].values():
        url = f'https://github.com/{UPSTREAM}/releases/download/v{manifest["version"]}/{spec["filename"]}'
        verified_asset(url, root/'downloads'/spec['filename'], spec['sha256'])
    license_data = api(f'repos/{UPSTREAM}/contents/LICENSE?ref=v{manifest["version"]}')
    license_text = base64.b64decode(license_data['content'], validate=False).decode('utf-8')
    if 'MIT License' not in license_text:
        raise ValueError('Upstream license changed; review it before preparing the release')
    writes = {}
    cargo = (root/'Cargo.toml').read_text()
    cargo, count = re.subn(r'(?m)^version = "'+re.escape(current)+r'"$', f'version = "{version}"', cargo, count=1)
    if count != 1:
        raise ValueError('Cannot locate project version in Cargo.toml')
    writes['Cargo.toml'] = cargo
    lock = (root/'Cargo.lock').read_text()
    marker = f'name = "token-peek"\nversion = "{current}"'
    if lock.count(marker) != 1:
        raise ValueError('Cargo.lock project version disagrees with Cargo.toml')
    writes['Cargo.lock'] = lock.replace(marker, f'name = "token-peek"\nversion = "{version}"')
    plist = plistlib.loads((root/'workflow/info.plist').read_bytes())
    if plist['version'] != current:
        raise ValueError('Alfred version disagrees with Cargo.toml')
    plist['version'] = version
    plist['readme'] = plist['readme'].replace(old['version'], manifest['version'])
    writes['workflow/info.plist'] = plistlib.dumps(plist, sort_keys=False).decode()
    for name in ('README.md', 'README.zh.md'):
        writes[name] = (root/name).read_text().replace(old['version'], manifest['version'])
    writes['vendor/manifest.json'] = json.dumps(manifest, indent=2)+'\n'
    writes['vendor/CodexBar-LICENSE'] = license_text
    writes['CHANGELOG.md'] = changelog
    for name, content in writes.items():
        (root/name).write_text(content)
    (root/'dist').mkdir(exist_ok=True)
    (root/'dist/RELEASE_NOTES.md').write_text(notes)
    print(f'Prepared v{version}; CodexBar {old["version"]} -> {manifest["version"]}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    commands.add_parser('upstream', help='Read latest stable metadata without changing files')
    prep = commands.add_parser('prepare', help='Update version, pinned CLI and Changelog; does not commit or publish')
    prep.add_argument('--version', required=True)
    notes = commands.add_parser('notes', help='Extract one version section for recovery/publishing')
    notes.add_argument('--version', required=True)
    args = parser.parse_args()
    if args.command == 'upstream':
        print(json.dumps(manifest_from_release(latest_release()), indent=2))
    elif args.command == 'notes':
        print(release_notes((ROOT/'CHANGELOG.md').read_text(), args.version), end='')
    else:
        current = tomllib.loads((ROOT/'Cargo.toml').read_text())['package']['version']
        validate_next_version(args.version, current)
        if subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT, text=True).strip():
            raise ValueError('Commit or stash tracked/untracked changes before release preparation')
        prepare(ROOT, args.version, latest_release())


if __name__ == '__main__':
    try:
        main()
    except (ValueError, KeyError, subprocess.CalledProcessError) as error:
        raise SystemExit(str(error)) from error
