import hashlib
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import tempfile
import unittest
import zipfile

SCRIPT = Path(__file__).resolve().parents[2] / 'scripts/install.sh'
BUNDLE = 'com.chang.token-peek'


class InstallTests(unittest.TestCase):
    def test_direct_install_and_update_preserve_settings_and_recover_from_failure(self):
        for scenario in ('install', 'update', 'checksum', 'download', 'old_macos', 'wrong_bundle', 'replace_failure', 'same', 'newer', 'numeric_update', 'update_failure',
                         'migrate6', 'migrate5', 'same_split', 'explicit5', 'terminal6', 'ambiguous', 'partial', 'bad_target'):
            with self.subTest(scenario=scenario), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                commands = root / 'bin'
                commands.mkdir()
                scratch = root / 'downloads'
                scratch.mkdir()
                preferences = root / 'Synced Preferences' / 'Alfred.alfredpreferences'
                workflows = preferences / 'workflows'
                workflows.mkdir(parents=True)
                existing = workflows / 'user.workflow.existing'
                latest = '0.10.0' if scenario == 'numeric_update' else '0.1.0'
                current = {'same': '0.1.0', 'newer': '0.2.0', 'numeric_update': '0.9.0'}.get(scenario, '0.0.1')
                split = scenario in ('migrate6', 'migrate5', 'same_split', 'explicit5', 'terminal6', 'ambiguous', 'partial', 'bad_target')
                major = '5' if scenario in ('migrate5', 'explicit5') else '6'
                if scenario in ('migrate5', 'migrate6', 'same_split'):
                    current = latest
                legacy = 'alfred-token-peek.alfredworkflow'
                asset = f'alfred-token-peek-alfred{major}.alfredworkflow' if split else legacy
                assets = [legacy]
                if split:
                    assets.append('alfred-token-peek-alfred5.alfredworkflow')
                    if scenario != 'partial':
                        assets.append('alfred-token-peek-alfred6.alfredworkflow')
                (root/'release.json').write_text(json.dumps({'tag_name':'v'+latest, 'assets':[{'name':a} for a in assets]}))
                if scenario != 'install':
                    existing.mkdir()
                    installed_major = {'migrate5':'6', 'migrate6':'5', 'same_split':major}.get(scenario)
                    installed_info = {'bundleid': BUNDLE, 'version': current}
                    if installed_major:
                        installed_info['variables'] = {'token_peek_alfred_major': installed_major}
                    (existing / 'info.plist').write_bytes(plistlib.dumps(installed_info))
                    (existing / 'prefs.plist').write_bytes(plistlib.dumps({'CLAUDE_ENABLED': False}))
                    (existing / 'old-file').write_text('old')
                if scenario in ('update', 'update_failure'):
                    shutil.copy2(SCRIPT, existing/'install.sh')
                    shutil.copy2(SCRIPT.parents[1]/'workflow/update.sh', existing/'update.sh')
                before = {str(p.relative_to(workflows)): p.read_bytes() for p in workflows.rglob('*') if p.is_file()}
                archive = root / 'package.zip'
                with zipfile.ZipFile(archive, 'w') as package:
                    package.writestr('info.plist', plistlib.dumps({
                        'bundleid': 'wrong' if scenario == 'wrong_bundle' else BUNDLE, 'version': latest,
                        'variables': {'token_peek_alfred_major':'5' if scenario == 'bad_target' else major}}))
                    for name in ('token-peek', 'action.sh'):
                        info = zipfile.ZipInfo(name)
                        info.external_attr = 0o100755 << 16
                        package.writestr(info, '#!/bin/bash\nexit 0\n')
                stubs = {
                    'uname': 'echo Darwin',
                    'sw_vers': 'echo ' + ('13.0' if scenario == 'old_macos' else '14.0'),
                    'open': 'exit 99',
                    'osascript': 'if [[ "$1" == -l ]]; then echo "$DETECTED"; exit; fi\nprintf "%s\\n" "${@: -1}" >> "$TEST_ROOT/notifications"',
                    'mv': '''
if [[ "$SCENARIO" == replace_failure && "$1" == *"/.token-peek."* ]]; then exit 1; fi
/bin/mv "$@"
''',
                    'curl': '''
if [[ "$*" == *releases/latest* ]]; then cat "$TEST_ROOT/release.json"; exit; fi
printf download >> "$TEST_ROOT/requests"
[[ "$SCENARIO" != download && "$SCENARIO" != update_failure ]] || exit 22
while [[ "$1" != -o ]]; do shift; done
if [[ "$2" == *SHA256SUMS ]]; then
  printf '%s  %s\\n' "$CHECKSUM" "$ASSET" > "$2"
else
  [[ "${2##*/}" == "$ASSET" ]] || exit 22
  cp "$TEST_ROOT/package.zip" "$2"
  [[ "$SCENARIO" != checksum ]] || printf corrupt >> "$2"
fi
''',
                }
                for name, body in stubs.items():
                    path = commands / name
                    path.write_text('#!/bin/bash\n' + body + '\n')
                    path.chmod(0o755)
                env = dict(os.environ, PATH=f'{commands}:/usr/bin:/bin', TMPDIR=str(scratch),
                           TEST_ROOT=str(root), SCENARIO=scenario, ASSET=asset, alfred_version=major,
                           DETECTED="" if scenario == "ambiguous" else "6.0.3", alfred_preferences=str(preferences),
                           CHECKSUM=hashlib.sha256(archive.read_bytes()).hexdigest())
                command = ['/bin/bash', str(SCRIPT)]
                if scenario in ('explicit5', 'terminal6', 'ambiguous'):
                    del env['alfred_version']
                if scenario == 'explicit5':
                    command.extend(['--alfred-major', '5'])
                if scenario in ('same', 'newer', 'numeric_update', 'migrate6', 'migrate5', 'same_split'):
                    command.append('--update')
                elif scenario in ('update', 'update_failure'):
                    command = ['/bin/bash', str(existing/'update.sh')]
                result = subprocess.run(command, env=env, capture_output=True, text=True)
                if scenario in ('install', 'update', 'numeric_update', 'migrate6', 'migrate5', 'explicit5', 'terminal6'):
                    self.assertEqual(result.returncode, 0, result.stderr)
                    installed, = workflows.iterdir()
                    self.assertEqual(plistlib.loads((installed / 'info.plist').read_bytes())['version'], latest)
                    self.assertTrue(os.access(installed / 'token-peek', os.X_OK))
                    if scenario in ('migrate6', 'migrate5', 'update'):
                        self.assertEqual(installed, existing)
                        self.assertEqual((installed / 'prefs.plist').read_bytes(), before['user.workflow.existing/prefs.plist'])
                        self.assertFalse((installed / 'old-file').exists())
                else:
                    if scenario in ('same', 'newer', 'same_split'):
                        self.assertEqual(result.returncode, 0, result.stderr)
                        self.assertFalse((root/'requests').exists())
                    else:
                        self.assertNotEqual(result.returncode, 0)
                        expected_errors = {'ambiguous':'--alfred-major', 'partial':'no package for Alfred 6',
                                           'bad_target':'Unexpected Alfred package variant'}
                        if scenario in expected_errors:
                            self.assertIn(expected_errors[scenario], result.stderr)
                    after = {str(p.relative_to(workflows)): p.read_bytes() for p in workflows.rglob('*') if p.is_file()}
                    self.assertEqual(before, after)
                    self.assertEqual(list(workflows.iterdir()), [existing])
                if scenario in ('update', 'update_failure'):
                    notifications = (root/'notifications').read_text()
                    self.assertIn('已更新至' if scenario == 'update' else '更新失败', notifications)
                self.assertEqual(list(scratch.iterdir()), [])
