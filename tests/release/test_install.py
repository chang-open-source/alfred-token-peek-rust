import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[2] / 'scripts/install.sh'


class InstallTests(unittest.TestCase):
    def test_verified_download_opens_alfred_and_failures_do_not(self):
        for scenario in ('success', 'checksum', 'download', 'old_macos'):
            with self.subTest(scenario=scenario), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                commands = root / 'bin'
                commands.mkdir()
                scratch = root / 'downloads'
                scratch.mkdir()
                checksum = hashlib.sha256(b'workflow').hexdigest()
                stubs = {
                    'uname': 'echo Darwin',
                    'sw_vers': 'echo ' + ('13.0' if scenario == 'old_macos' else '14.0'),
                    'plutil': 'cat >/dev/null; echo v0.1.0',
                    'open': 'printf "%s" "$1" > "$TEST_ROOT/opened"',
                    'curl': '''
if [[ "$*" == *releases/latest* ]]; then echo '{}'; exit; fi
[[ "$SCENARIO" != download ]] || exit 22
while [[ "$1" != -o ]]; do shift; done
if [[ "$2" == *SHA256SUMS ]]; then
  printf '%s  alfred-token-peek.alfredworkflow\\n' "$CHECKSUM" > "$2"
else
  printf workflow > "$2"
  [[ "$SCENARIO" != checksum ]] || printf corrupt >> "$2"
fi
''',
                }
                for name, body in stubs.items():
                    path = commands / name
                    path.write_text('#!/bin/bash\n' + body + '\n')
                    path.chmod(0o755)
                env = dict(os.environ, PATH=f'{commands}:/usr/bin:/bin', TMPDIR=str(scratch),
                           TEST_ROOT=str(root), SCENARIO=scenario, CHECKSUM=checksum)
                result = subprocess.run(['/bin/bash', str(SCRIPT)], env=env, capture_output=True, text=True)
                opened = root / 'opened'
                if scenario == 'success':
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(Path(opened.read_text()).read_bytes(), b'workflow')
                else:
                    self.assertNotEqual(result.returncode, 0)
                    self.assertFalse(opened.exists())
                    self.assertEqual(list(scratch.iterdir()), [])
