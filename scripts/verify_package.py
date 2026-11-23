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
dist = ROOT/'dist'
archive = dist/'alfred-token-peek-alfred6.alfredworkflow'
list_archive = dist/'alfred-token-peek-alfred5.alfredworkflow'
legacy = dist/'alfred-token-peek.alfredworkflow'
sums = {name: digest for digest, name in (line.split() for line in (dist/'SHA256SUMS').read_text().splitlines())}
for path in (archive, list_archive, legacy):
    assert hashlib.sha256(path.read_bytes()).hexdigest() == sums[path.name], path
assert sums[list_archive.name] == sums[legacy.name]
with zipfile.ZipFile(list_archive) as five, zipfile.ZipFile(archive) as six:
    p5 = plistlib.loads(five.read('info.plist'))
    p6 = plistlib.loads(six.read('info.plist'))
    assert p5['bundleid'] == p6['bundleid'] and p5['version'] == p6['version']
    assert p5['variables']['token_peek_alfred_major'] == '5' and p6['variables']['token_peek_alfred_major'] == '6'
    assert {o['type'] for o in p5['objects']} == {'alfred.workflow.input.scriptfilter', 'alfred.workflow.action.script'}
    uids = {o['uid'] for o in p5['objects']}
    assert set(p5['uidata']) == uids and set(p5['connections']) <= uids
    assert all(link['destinationuid'] in uids for links in p5['connections'].values() for link in links)
    assert p5['connections']['token-peek-filter'] == [link for link in p6['connections']['token-peek-filter'] if link['modifiers'] == 0]
    assert set(five.namelist()) == {name for name in six.namelist() if not name.startswith('Quota.alfredui/')}
    for entry in five.infolist():
        if entry.filename != 'info.plist':
            assert entry.external_attr == six.getinfo(entry.filename).external_attr
            assert five.read(entry) == six.read(entry.filename), entry.filename
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
    versions={'alfred.workflow.input.scriptfilter':3,'alfred.workflow.action.script':2,
              'alfred.workflow.userinterface.alfredui':1}
    assert all(obj['version']==versions[obj['type']] for obj in plist['objects'])
    objects = {obj['uid']: obj for obj in plist['objects']}
    connections = plist['connections']['token-peek-filter']
    details_link, = [link for link in connections if link['modifiers'] == 1 << 19]
    details = objects[details_link['destinationuid']]
    assert details['type'] == 'alfred.workflow.userinterface.alfredui'
    [detail_action] = plist['connections'][details['uid']]
    assert objects[detail_action['destinationuid']]['config']['script'] == './action.sh "$1"'
    ui_path = details['config']['path'] + '/ui.json'
    assert z.read(ui_path) == (ROOT/'workflow'/ui_path).read_bytes()
    ui = json.loads(z.read(ui_path))
    # Alfred 6.0.3 centres button symbols over their titles. Keep these actions text-only.
    controls = [child for group in ui['window']['root']['components']
                for child in group.get('components', []) if child['type'] == 'button']
    assert [button['text'] for button in controls] == ['复制摘要', '刷新此服务', '打开用量页']
    assert all('image' not in button for button in controls), 'Overlapping quota button icons'
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
    fake.write_text("#!/bin/sh\nsleep 0.1\nprintf '%s\\n' '[{\"provider\":\"codex\",\"usage\":{\"primary\":{\"usedPercent\":28},\"secondary\":{\"usedPercent\":100},\"tertiary\":{}}},{\"provider\":\"claude\",\"usage\":{\"primary\":{\"usedPercent\":50}}}]'\n")
    fake.chmod(0o700)
    env=dict(os.environ,alfred_version='6.0.3',CLAUDE_ENABLED='false',CODEXBAR_BIN=str(fake),alfred_workflow_cache=str(root/'cache'))
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
    quota = next(row for row in view['items'] if row.get('uid') == 'codex-0')
    variables = quota['mods']['alt']['variables']
    for name, config in ui['variablesconfig'].items():
        if config['src'] == 'args[0]':
            assert quota['mods']['alt']['arg'] == 'codex'
        else:
            assert config['src'] == 'variable' and name in variables, name
    # Exercise the packaged preload action with Alfred's string-valued workflow variables.
    panel = ui['window']['root']
    event = panel['events']
    assert event['triggers'] == 'preload'
    action = event['action']
    meter, = [component for component in panel['components'] if component['type'] == 'progress']
    for uid, fraction, hidden in [('codex-0', 0.72, False), ('codex-1', 0.0, False), ('codex-2', 0.0, True)]:
        row = next(row for row in view['items'] if row.get('uid') == uid)
        meter_vars = row['mods']['alt']['variables']
        args = [meter_vars[arg[5:-1]] if arg.startswith('{var:') else arg for arg in action['args']]
        update = json.loads(subprocess.check_output([action['src'], *args], cwd=root/'Quota.alfredui'))
        values = update['update']['ids'][meter['id']]
        assert values['progress'] == fraction and values['hidden'] is hidden, values
    assert not any(row.get('arg')=='claude' for row in view['items'])
    assert (root/'cache/codex/usage.json').stat().st_mode & 0o777==0o600
    fake.write_text('#!/bin/sh\nexit 1\n')
    subprocess.run([str(root/'token-peek'),'refresh'],env=env,cwd=root,check=True)
    view=query('codex')
    assert 'rerun' not in view
    assert any('72%' in row['title'] for row in view['items'])
    assert '缓存已过期' in json.dumps(view,ensure_ascii=False)
print('PASS: archive, dual-arch CLIs, permissions, spaced paths, filter/worker, failed refresh retention')
