#!/usr/bin/env python3
"""Check the actual archive, then exercise its filter/worker with a fake CLI."""
import hashlib
import json
import os
import platform
import re
import tomllib
from pathlib import Path
import plistlib
import subprocess
import tempfile
import time
import zipfile
from macos import validate_minimum_os

ROOT=Path(__file__).resolve().parents[1]
project_version=tomllib.loads((ROOT/'Cargo.toml').read_text())['package']['version']
manifest=json.loads((ROOT/'vendor/manifest.json').read_text())
archive=ROOT/'dist/alfred-token-peek.alfredworkflow'
assert hashlib.sha256(archive.read_bytes()).hexdigest()==(ROOT/'dist/SHA256SUMS').read_text().split()[0]
with zipfile.ZipFile(archive) as z:
    assert z.testzip() is None
    plist=plistlib.loads(z.read('info.plist'))
    assert plist['bundleid']=='com.chang.token-peek'
    assert plist['version']==project_version
    assert json.loads(z.read('vendor/manifest.json'))==manifest
    for name in ['README.md', 'README.zh.md']:
        assert z.read(name)==(ROOT/name).read_bytes()
    assert z.read('CHANGELOG.md')==(ROOT/'CHANGELOG.md').read_bytes()
    assert z.read('docs/releasing.md')==(ROOT/'docs/releasing.md').read_bytes()
    versions={'alfred.workflow.input.scriptfilter':3,'alfred.workflow.action.script':2}
    assert all(obj['version']==versions[obj['type']] for obj in plist['objects'])
    for arch in ['arm64','x86_64']:
        for name in ['CodexBarCLI','LICENSE','THIRD_PARTY_NOTICES','CodexBar_CodexBarCore.bundle/Contents/Info.plist']:
            assert f'vendor/{arch}/{name}' in z.namelist()
        assert z.getinfo(f'vendor/{arch}/CodexBarCLI').external_attr >> 16 & 0o111
    assert z.getinfo('token-peek').external_attr >> 16 & 0o111
    for name, source in [('action.sh', 'workflow/action.sh'), ('update.sh', 'workflow/update.sh'), ('install.sh', 'scripts/install.sh')]:
        assert z.getinfo(name).external_attr >> 16 & 0o111
        assert z.read(name)==(ROOT/source).read_bytes()
with tempfile.TemporaryDirectory(prefix='Token Peek package ') as folder:
    root=Path(folder)
    subprocess.run(['unzip','-q',str(archive),'-d',str(root)],check=True)
    arches=subprocess.check_output(['lipo','-archs',str(root/'token-peek')],text=True).split()
    assert set(arches)=={'arm64','x86_64'},arches
    assert subprocess.check_output([str(root/'token-peek'),'--version'],text=True).strip()==f'token-peek {project_version}'
    for arch in ['arm64','x86_64']:
        cli=root/f'vendor/{arch}/CodexBarCLI'
        slices=subprocess.check_output(['lipo','-archs',str(cli)],text=True).split()
        assert arch in slices, (arch,slices)
        validate_minimum_os(subprocess.check_output(['otool','-l',str(cli)],text=True))
        # Intel runners cannot execute arm64; run the native CLI, inspect both architectures.
        if arch == platform.machine():
            output=subprocess.check_output([str(cli),'--version'],text=True)
            assert re.search(r'(?<![\d.])'+re.escape(manifest['version'])+r'(?![\d.])',output),output
    fake=root/'fake CLI'
    fake.write_text("#!/bin/sh\nsleep 0.1\nprintf '%s\\n' '[{\"provider\":\"codex\",\"usage\":{\"primary\":{\"usedPercent\":28}}},{\"provider\":\"claude\",\"usage\":{\"primary\":{\"usedPercent\":50}}}]'\n")
    fake.chmod(0o700)
    env=dict(os.environ,CLAUDE_ENABLED='false',CODEXBAR_BIN=str(fake),alfred_workflow_cache=str(root/'cache'))
    def query(q=''):
        v=json.loads(subprocess.check_output([str(root/'token-peek'),'filter',q],cwd=root,env=env,text=True))
        env.update(v.get('variables',{}))
        return v
    update=query('update')
    assert update['items'][0]['arg']=='update' and 'rerun' not in update
    first=query()
    assert first['rerun']==1
    for _ in range(60):
        time.sleep(.1)
        view=query('codex')
        if 'rerun' not in view:break
    else:raise AssertionError('worker did not finish')
    assert any('72%' in row['title'] for row in view['items']),view
    assert not any(row.get('arg')=='claude' for row in view['items'])
    assert (root/'cache/codex/usage.json').stat().st_mode & 0o777==0o600
    fake.write_text('#!/bin/sh\nexit 1\n')
    subprocess.run([str(root/'token-peek'),'refresh'],env=env,cwd=root,check=True)
    view=query('codex')
    assert 'rerun' not in view
    assert any('72%' in row['title'] for row in view['items'])
    assert '缓存已过期' in json.dumps(view,ensure_ascii=False)
print('PASS: archive, dual-arch CLIs, permissions, spaced paths, filter/worker, failed refresh retention')
