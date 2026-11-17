#!/usr/bin/env python3
"""Fetch pinned official CLI archives; reject changed bytes before extraction."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[1]

def main():
    manifest = json.loads((ROOT / 'vendor/manifest.json').read_text())
    downloads = ROOT / 'downloads'
    downloads.mkdir(exist_ok=True)
    for arch, spec in manifest['archives'].items():
        archive = downloads / spec['filename']
        if not archive.exists():
            url = f"https://github.com/steipete/CodexBar/releases/download/v{manifest['version']}/{archive.name}"
            partial = archive.with_suffix('.partial')
            subprocess.run(['curl', '-fL', '--retry', '2', '--max-time', '180', url, '-o', str(partial)], check=True)
            partial.replace(archive)
        actual = hashlib.sha256(archive.read_bytes()).hexdigest()
        if actual != spec['sha256']:
            raise SystemExit(f'SHA256 mismatch: {archive.name}; remove the archive and retry')
        dest = ROOT / 'vendor/runtime' / arch
        if dest.exists():
            shutil.rmtree(dest)
        dest.mkdir(parents=True)
        with tarfile.open(archive) as tar:
            # Python 3.12+ data filter also validates symlink destinations.
            if hasattr(tarfile, 'data_filter'):
                tar.extractall(dest, filter='data')
            else:
                raise SystemExit('Packaging requires Python 3.12+ for safe archive extraction')
        shutil.copy2(ROOT / 'vendor/CodexBar-LICENSE', dest / 'LICENSE')
        print(f'Verified and extracted {arch}: {actual}')

if __name__ == '__main__':
    main()
