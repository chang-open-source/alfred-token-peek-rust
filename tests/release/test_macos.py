import sys
from pathlib import Path
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from macos import validate_minimum_os

class MacOSTests(unittest.TestCase):
    def test_rejects_upstream_requiring_a_newer_macos(self):
        for minos in ('14.1', '15.0'):
            with self.subTest(minos=minos), self.assertRaises(ValueError):
                validate_minimum_os(f'Load command 1\n      cmd LC_BUILD_VERSION\n platform 1\n    minos {minos}\n      sdk 15.0\nLoad command 2\n cmd LC_UUID\n')

    def test_accepts_supported_build_and_legacy_minimum_commands(self):
        validate_minimum_os('cmd LC_BUILD_VERSION\n platform 1\n minos 14.0\n sdk 15.0\n')
        validate_minimum_os('cmd LC_VERSION_MIN_MACOSX\n version 13.0\n sdk 14.0\n')

    def test_rejects_missing_or_non_macos_minimum(self):
        for commands in ('cmd LC_UUID\n', 'cmd LC_BUILD_VERSION\n platform 2\n minos 14.0\n'):
            with self.subTest(commands=commands), self.assertRaises(ValueError):
                validate_minimum_os(commands)
