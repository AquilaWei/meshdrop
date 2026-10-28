"""Tests for release inputs and fail-closed artifact staging."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import release


class ReleaseTests(unittest.TestCase):
    def test_notes_select_exact_version_not_prefix(self):
        self.assertEqual(release.release_notes(
            '# Changes\n## 0.2.01\nWrong\n## 0.2.0 - date\n\nRight\n## 0.1.0\nOld\n',
            '0.2.0'), 'Right\n')

    def test_bracketed_changelog_heading_is_supported(self):
        self.assertEqual(release.release_notes('## [0.2.0] - date\nNotes\n', '0.2.0'), 'Notes\n')

    def test_absent_notes_fail(self):
        with self.assertRaises(ValueError):
            release.release_notes('## 0.1.0\nOld\n', '0.2.0')

    def test_empty_notes_fail(self):
        with self.assertRaises(ValueError):
            release.release_notes('## 0.2.0\n\n', '0.2.0')

    def test_duplicate_notes_fail(self):
        with self.assertRaises(ValueError):
            release.release_notes('## 0.2.0\nA\n## 0.2.0\nB\n', '0.2.0')

    def test_mismatched_tag_fails_before_writing_notes(self):
        with patch.object(release, 'version', return_value='0.2.0'):
            with self.assertRaisesRegex(ValueError, 'Tag must exactly match'):
                release.metadata('v0.2.1')

    def test_root_workspace_is_version_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'Cargo.toml').write_text('[workspace.package]\nversion = "0.2.0"\n')
            self.assertEqual(release.version(root), '0.2.0')

    def test_prerelease_version_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'Cargo.toml').write_text('[workspace.package]\nversion = "0.2.0-rc1"\n')
            with self.assertRaises(ValueError):
                release.version(root)

    def test_missing_installer_fails(self):
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaises(ValueError):
                release.collect('apk', temporary, temporary)

    def test_ambiguous_installer_fails(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'one.apk').write_bytes(b'one')
            (root / 'two.apk').write_bytes(b'two')
            with self.assertRaises(ValueError):
                release.collect('apk', root, root / 'out')

    def test_dry_run_installer_is_labelled(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'app.apk').write_bytes(b'apk')
            with patch.object(release, 'version', return_value='0.2.0'):
                release.collect('apk', root, root / 'out', True)
            self.assertEqual((root / 'out' / 'meshdrop-0.2.0-arm64-NOT-FOR-DISTRIBUTION.apk').read_bytes(), b'apk')

    def test_incomplete_installers_cannot_produce_checksums(self):
        with tempfile.TemporaryDirectory() as temporary:
            (Path(temporary) / 'meshdrop-0.2.0-arm64.apk').write_bytes(b'apk')
            with self.assertRaises(ValueError):
                release.checksums(temporary)

    def test_checksums_cover_exact_three_installer_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'meshdrop-0.2.0-arm64.apk').write_bytes(b'')
            (root / 'meshdrop-0.2.0-universal.dmg').write_bytes(b'')
            (root / 'meshdrop-0.2.0-x86_64.rpm').write_bytes(b'')
            with patch.object(release, 'version', return_value='0.2.0'):
                release.checksums(root)
            self.assertEqual((root / 'SHA256SUMS').read_text(),
                'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  meshdrop-0.2.0-arm64.apk\n'
                'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  meshdrop-0.2.0-universal.dmg\n'
                'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  meshdrop-0.2.0-x86_64.rpm\n')

    def test_stale_version_artifact_fails(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'meshdrop-0.1.0-arm64.apk').write_bytes(b'')
            (root / 'meshdrop-0.2.0-universal.dmg').write_bytes(b'')
            (root / 'meshdrop-0.2.0-x86_64.rpm').write_bytes(b'')
            with patch.object(release, 'version', return_value='0.2.0'):
                with self.assertRaises(ValueError):
                    release.checksums(root)

    def test_tag_signing_without_secrets_fails(self):
        result = subprocess.run(['bash', str(release.ROOT / 'scripts/packaging/signing.sh')],
                                env={'PATH': os.environ['PATH'], 'GITHUB_EVENT_NAME': 'push'},
                                capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Tag builds require the release keystore secret', result.stderr)


if __name__ == '__main__':
    unittest.main()
