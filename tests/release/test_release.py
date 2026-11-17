import json
import base64
import hashlib
import plistlib
from unittest.mock import patch
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
import release


def upstream(version='0.85.0'):
    return {'tag_name': 'v'+version, 'draft': False, 'prerelease': False,
            'html_url': f'https://github.com/steipete/CodexBar/releases/tag/v{version}',
            'assets': [{'name': f'CodexBarCLI-v{version}-macos-{arch}.tar.gz',
                        'digest': 'sha256:'+'a'*64} for arch in ('arm64', 'x86_64')]}


class ReleaseTests(unittest.TestCase):
    def test_bump_increments_selected_component_and_resets_lower_components(self):
        for bump, expected in [('patch', '1.2.10'), ('minor', '1.3.0'), ('major', '2.0.0')]:
            self.assertEqual(release.bump_version('1.2.9', bump), expected)
        with self.assertRaises(ValueError):
            release.bump_version('1.2.9', 'custom')

    def test_rejects_invalid_or_non_increasing_versions(self):
        for version in ('v1.0.0', '1.0', '01.2.3', '1.0.0-beta', '1.0.0\nx', '$(oops)'):
            with self.subTest(version=version), self.assertRaises(ValueError):
                release.version_tuple(version)
        for version in ('0.3.2', '0.3.1'):
            with self.assertRaises(ValueError):
                release.validate_next_version(version, '0.3.2')
        release.validate_next_version('0.4.0', '0.3.2')

    def test_release_requires_stable_dual_arch_assets_and_digests(self):
        manifest = release.manifest_from_release(upstream())
        self.assertEqual(manifest['version'], '0.85.0')
        self.assertEqual(set(manifest['archives']), {'arm64', 'x86_64'})
        variants = []
        for field in ('draft', 'prerelease'):
            value = upstream(); value[field] = True; variants.append(value)
        value = upstream(); value['assets'].pop(); variants.append(value)
        value = upstream(); value['assets'][0]['digest'] = None; variants.append(value)
        value = upstream(); value['assets'][0]['digest'] = 'sha256:bad'; variants.append(value)
        value = upstream(); value['assets'].append(value['assets'][0]); variants.append(value)
        for value in variants:
            with self.subTest(value=value), self.assertRaises(ValueError):
                release.manifest_from_release(value)

    def test_changelog_consumes_unreleased_and_records_upstream_upgrade(self):
        original = '# Changelog\n\n## [Unreleased]\n\n- 本项目修复。\n\n## [0.3.2] - 2026-11-17\n\n- 历史内容\n'
        updated, notes = release.render_changelog(original, '0.4.0', '2026-11-17', '0.84.0', upstream())
        self.assertNotIn('## [Unreleased]', updated)
        self.assertIn('本项目修复', notes)
        self.assertIn('0.84.0 → 0.85.0', notes)
        self.assertIn('/compare/v0.84.0...v0.85.0', notes)
        self.assertNotIn('历史内容', notes)
        self.assertFalse(updated.endswith('\n\n'))
        with self.assertRaises(ValueError):
            release.render_changelog(updated, '0.4.0', '2026-11-17', '0.85.0', upstream())

    def test_release_without_pending_changes(self):
        history = '## [0.1.0]\n\n- History\n'
        for pending in ('', '## [Unreleased]\n\n'):
            updated, notes = release.render_changelog(
                '# Changelog\n\n'+pending+history, '0.2.0', '2026-11-17', '0.85.0', upstream())
            self.assertNotIn('Unreleased', updated)
            self.assertTrue(updated.endswith(history))
            self.assertEqual(release.release_notes(updated, '0.2.0'), notes)
            self.assertNotIn('→', notes)

    def test_checksum_mismatch_does_not_replace_cached_asset(self):
        with tempfile.TemporaryDirectory() as tmp:
            dest = Path(tmp)/'asset.tar.gz'; dest.write_bytes(b'bad')
            with patch.object(release, 'download', side_effect=AssertionError('must not download')):
                with self.assertRaises(ValueError):
                    release.verified_asset('https://example.invalid/file', dest, 'a'*64)
            self.assertEqual(dest.read_bytes(), b'bad')

    def test_notes_are_extracted_from_exact_changelog_section(self):
        doc = '# Changelog\n\n## [Unreleased]\n\n- Next\n\n## [0.4.0] - 2026-11-17\n\n### Fixed\n- Fix\n\n## [0.3.2]\n- Old\n'
        self.assertEqual(release.release_notes(doc, '0.4.0'), '### Fixed\n- Fix\n')
        with self.assertRaises(ValueError):
            release.release_notes(doc, '9.0.0')

    def test_prepare_synchronizes_versions_and_failure_leaves_tracked_files_unchanged(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            for name in ('vendor', 'workflow'):
                (root/name).mkdir()
            files = {
                'Cargo.toml': '[package]\nname = "token-peek"\nversion = "0.3.2"\n',
                'Cargo.lock': '[[package]]\nname = "token-peek"\nversion = "0.3.2"\n',
                'vendor/manifest.json': json.dumps({'version': '0.84.0'}),
                'CHANGELOG.md': '# Changelog\n\n## [Unreleased]\n\n- Fix\n',
                'README.md': 'CodexBar 0.84.0',
                'README.zh.md': '内置 CodexBar 0.84.0',
                'vendor/CodexBar-LICENSE': 'MIT License old',
            }
            for name, content in files.items():
                (root/name).write_text(content)
            (root/'workflow/info.plist').write_bytes(plistlib.dumps({'version':'0.3.2','readme':'CodexBar 0.84.0'}))
            before = {str(p.relative_to(root)):p.read_bytes() for p in root.rglob('*') if p.is_file()}
            with patch.object(release, 'verified_asset', side_effect=ValueError('bad digest')):
                with self.assertRaises(ValueError):
                    release.prepare(root, '0.4.0', upstream())
            self.assertEqual(before, {str(p.relative_to(root)):p.read_bytes() for p in root.rglob('*') if p.is_file()})
            license_data = {'content': base64.b64encode(b'MIT License new').decode()}
            with patch.object(release, 'verified_asset'), patch.object(release, 'api', return_value=license_data):
                release.prepare(root, '0.4.0', upstream())
            self.assertIn('version = "0.4.0"', (root/'Cargo.lock').read_text())
            self.assertEqual(plistlib.loads((root/'workflow/info.plist').read_bytes())['version'], '0.4.0')
            self.assertEqual((root/'README.md').read_text(), 'CodexBar 0.85.0')
            self.assertEqual((root/'README.zh.md').read_text(), '内置 CodexBar 0.85.0')
            self.assertEqual((root/'vendor/CodexBar-LICENSE').read_text(), 'MIT License new')
            self.assertEqual((root/'dist/RELEASE_NOTES.md').read_text(), release.release_notes((root/'CHANGELOG.md').read_text(), '0.4.0'))

    def test_new_download_is_verified_before_becoming_cached(self):
        with tempfile.TemporaryDirectory() as folder:
            dest = Path(folder)/'asset'
            def fake_download(url, path):
                path.write_bytes(b'valid')
            with patch.object(release, 'download', side_effect=fake_download):
                with self.assertRaises(ValueError):
                    release.verified_asset('unused', dest, 'a'*64)
                self.assertFalse(dest.exists())
                self.assertEqual(list(Path(folder).iterdir()), [])
                release.verified_asset('unused', dest, hashlib.sha256(b'valid').hexdigest())
            self.assertEqual(dest.read_bytes(), b'valid')

if __name__ == '__main__':
    unittest.main()
