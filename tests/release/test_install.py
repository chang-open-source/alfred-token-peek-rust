import hashlib
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
        for scenario in ('install', 'update', 'checksum', 'download', 'old_macos', 'wrong_bundle', 'replace_failure', 'same', 'newer', 'numeric_update', 'update_failure'):
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
                if scenario != 'install':
                    existing.mkdir()
                    (existing / 'info.plist').write_bytes(plistlib.dumps({'bundleid': BUNDLE, 'version': current}))
                    (existing / 'prefs.plist').write_bytes(plistlib.dumps({'CLAUDE_ENABLED': False}))
                    (existing / 'old-file').write_text('old')
                if scenario in ('update', 'update_failure'):
                    shutil.copy2(SCRIPT, existing/'install.sh')
                    shutil.copy2(SCRIPT.parents[1]/'workflow/update.sh', existing/'update.sh')
                before = {str(p.relative_to(workflows)): p.read_bytes() for p in workflows.rglob('*') if p.is_file()}
                archive = root / 'package.zip'
                with zipfile.ZipFile(archive, 'w') as package:
                    package.writestr('info.plist', plistlib.dumps({
                        'bundleid': 'wrong' if scenario == 'wrong_bundle' else BUNDLE, 'version': latest}))
                    for name in ('token-peek', 'action.sh'):
                        info = zipfile.ZipInfo(name)
                        info.external_attr = 0o100755 << 16
                        package.writestr(info, '#!/bin/bash\nexit 0\n')
                stubs = {
                    'uname': 'echo Darwin',
                    'sw_vers': 'echo ' + ('13.0' if scenario == 'old_macos' else '14.0'),
                    'open': 'exit 99',
                    'osascript': 'printf "%s\\n" "${@: -1}" >> "$TEST_ROOT/notifications"',
                    'mv': '''
if [[ "$SCENARIO" == replace_failure && "$1" == *"/.token-peek."* ]]; then exit 1; fi
/bin/mv "$@"
''',
                    'curl': '''
if [[ "$*" == *releases/latest* ]]; then printf '{"tag_name":"v%s"}' "$LATEST"; exit; fi
printf download >> "$TEST_ROOT/requests"
[[ "$SCENARIO" != download && "$SCENARIO" != update_failure ]] || exit 22
while [[ "$1" != -o ]]; do shift; done
if [[ "$2" == *SHA256SUMS ]]; then
  printf '%s  alfred-token-peek.alfredworkflow\\n' "$CHECKSUM" > "$2"
else
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
                           TEST_ROOT=str(root), SCENARIO=scenario, LATEST=latest, alfred_preferences=str(preferences),
                           CHECKSUM=hashlib.sha256(archive.read_bytes()).hexdigest())
                command = ['/bin/bash', str(SCRIPT)]
                if scenario in ('same', 'newer', 'numeric_update'):
                    command.append('--update')
                elif scenario in ('update', 'update_failure'):
                    command = ['/bin/bash', str(existing/'update.sh')]
                result = subprocess.run(command, env=env, capture_output=True, text=True)
                if scenario in ('install', 'update', 'numeric_update'):
                    self.assertEqual(result.returncode, 0, result.stderr)
                    installed, = workflows.iterdir()
                    self.assertEqual(plistlib.loads((installed / 'info.plist').read_bytes())['version'], latest)
                    self.assertTrue(os.access(installed / 'token-peek', os.X_OK))
                    if scenario == 'update':
                        self.assertEqual(installed, existing)
                        self.assertEqual((installed / 'prefs.plist').read_bytes(), before['user.workflow.existing/prefs.plist'])
                        self.assertFalse((installed / 'old-file').exists())
                else:
                    if scenario in ('same', 'newer'):
                        self.assertEqual(result.returncode, 0, result.stderr)
                        self.assertFalse((root/'requests').exists())
                    else:
                        self.assertNotEqual(result.returncode, 0)
                    after = {str(p.relative_to(workflows)): p.read_bytes() for p in workflows.rglob('*') if p.is_file()}
                    self.assertEqual(before, after)
                    self.assertEqual(list(workflows.iterdir()), [existing])
                if scenario in ('update', 'update_failure'):
                    notifications = (root/'notifications').read_text()
                    self.assertIn('已更新至' if scenario == 'update' else '更新失败', notifications)
                self.assertEqual(list(scratch.iterdir()), [])
