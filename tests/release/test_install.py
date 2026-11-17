import hashlib
import os
from pathlib import Path
import plistlib
import subprocess
import tempfile
import unittest
import zipfile

SCRIPT = Path(__file__).resolve().parents[2] / 'scripts/install.sh'
BUNDLE = 'com.chang.token-peek'


class InstallTests(unittest.TestCase):
    def test_direct_install_and_update_preserve_settings_and_recover_from_failure(self):
        for scenario in ('install', 'update', 'checksum', 'download', 'old_macos', 'wrong_bundle', 'replace_failure'):
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
                if scenario != 'install':
                    existing.mkdir()
                    (existing / 'info.plist').write_bytes(plistlib.dumps({'bundleid': BUNDLE, 'version': '0.0.1'}))
                    (existing / 'prefs.plist').write_bytes(plistlib.dumps({'CLAUDE_ENABLED': False}))
                    (existing / 'old-file').write_text('old')
                before = {str(p.relative_to(workflows)): p.read_bytes() for p in workflows.rglob('*') if p.is_file()}
                archive = root / 'package.zip'
                with zipfile.ZipFile(archive, 'w') as package:
                    package.writestr('info.plist', plistlib.dumps({
                        'bundleid': 'wrong' if scenario == 'wrong_bundle' else BUNDLE, 'version': '0.1.0'}))
                    for name in ('token-peek', 'action.sh'):
                        info = zipfile.ZipInfo(name)
                        info.external_attr = 0o100755 << 16
                        package.writestr(info, '#!/bin/bash\nexit 0\n')
                stubs = {
                    'uname': 'echo Darwin',
                    'sw_vers': 'echo ' + ('13.0' if scenario == 'old_macos' else '14.0'),
                    'open': 'exit 99',
                    'mv': '''
if [[ "$SCENARIO" == replace_failure && "$1" == *"/.token-peek."* ]]; then exit 1; fi
/bin/mv "$@"
''',
                    'curl': '''
if [[ "$*" == *releases/latest* ]]; then echo '{"tag_name":"v0.1.0"}'; exit; fi
[[ "$SCENARIO" != download ]] || exit 22
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
                           TEST_ROOT=str(root), SCENARIO=scenario, alfred_preferences=str(preferences),
                           CHECKSUM=hashlib.sha256(archive.read_bytes()).hexdigest())
                result = subprocess.run(['/bin/bash', str(SCRIPT)], env=env, capture_output=True, text=True)
                if scenario in ('install', 'update'):
                    self.assertEqual(result.returncode, 0, result.stderr)
                    installed, = workflows.iterdir()
                    self.assertEqual(plistlib.loads((installed / 'info.plist').read_bytes())['version'], '0.1.0')
                    self.assertTrue(os.access(installed / 'token-peek', os.X_OK))
                    if scenario == 'update':
                        self.assertEqual(installed, existing)
                        self.assertEqual((installed / 'prefs.plist').read_bytes(), before['user.workflow.existing/prefs.plist'])
                        self.assertFalse((installed / 'old-file').exists())
                else:
                    self.assertNotEqual(result.returncode, 0)
                    after = {str(p.relative_to(workflows)): p.read_bytes() for p in workflows.rglob('*') if p.is_file()}
                    self.assertEqual(before, after)
                    self.assertEqual(list(workflows.iterdir()), [existing])
                self.assertEqual(list(scratch.iterdir()), [])
