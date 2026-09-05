"""Release metadata contracts; standard library only."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from release_assets import package_version, changelog, copy_legion_proof, verify_archives, write_checksums, TARGETS


class ReleaseAssets(unittest.TestCase):
    def test_cargo_and_lock_versions_and_exact_tag(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'Cargo.toml').write_text('[package]\nname="ooxml-cli"\nversion="1.2.3"\n')
            (root / 'Cargo.lock').write_text('version=4\n[[package]]\nname="ooxml-cli"\nversion="1.2.3"\n')
            self.assertEqual(package_version(root, 'v1.2.3'), '1.2.3')
            self.assertEqual(package_version(root), '1.2.3')
            for tag in ['1.2.3', 'v1.2.4', 'v1.2.3\nextra=bad']:
                with self.assertRaises(ValueError):
                    package_version(root, tag)
            (root / 'Cargo.lock').write_text('version=4\n[[package]]\nname="ooxml-cli"\nversion="1.2.4"\n')
            with self.assertRaisesRegex(ValueError, 'disagree'):
                package_version(root)

    def test_changelog_uses_only_newly_closed_beads_in_id_order(self):
        previous = json.dumps({'id': 'old', 'status': 'closed', 'title': 'Old'})
        rows = [
            {'id': 'z', 'status': 'closed', 'title': 'Last'},
            {'id': 'old', 'status': 'closed', 'title': 'Old'},
            {'id': 'open', 'status': 'open', 'title': 'Undelivered'},
            {'id': 'pending', 'status': 'batch_pending', 'title': 'Unverified'},
            {'id': 'a', 'status': 'closed', 'title': 'First\nline <tag>'},
        ]
        text = '\n'.join(map(json.dumps, rows))
        expected = '# v1.2.3\n\nCompleted beads since the previous release tag:\n\n- `a`: First line &lt;tag&gt;\n- `z`: Last\n'
        self.assertEqual(changelog(text, previous, 'v1.2.3'), expected)
        self.assertEqual(changelog('\n'.join(map(json.dumps, reversed(rows))), previous, 'v1.2.3'), expected)
        self.assertIn('No newly closed', changelog(previous, previous, 'v1.2.3'))

    def test_archive_inventory_and_checksum_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            with self.assertRaisesRegex(ValueError, 'exactly four'):
                verify_archives(dist, 'v1.2.3')
            # Unit inputs exercise inventory checks, not native binary/architecture proof.
            for target, suffix in TARGETS:
                (dist / f'ooxml-v1.2.3-{target}.{suffix}').write_bytes(b'archive inventory input')
            verify_archives(dist, 'v1.2.3')
            first = sorted(dist.iterdir())[0]
            first.write_bytes(b'')
            with self.assertRaisesRegex(ValueError, 'empty'):
                verify_archives(dist, 'v1.2.3')
            first.write_bytes(b'archive inventory input')
            (dist / 'unexpected.txt').write_text('unexpected')
            with self.assertRaisesRegex(ValueError, 'unexpected'):
                verify_archives(dist, 'v1.2.3')
            (dist / 'unexpected.txt').unlink()
            (dist / 'capabilities.json').write_bytes(b'{"commands":[]}\n')
            (dist / 'CHANGELOG.md').write_bytes(b'# v1.2.3\n')
            write_checksums(dist)
            checksum_bytes = (dist / 'SHA256SUMS').read_bytes()
            self.assertNotIn(b'\r', checksum_bytes)
            lines = checksum_bytes.decode().splitlines()
            self.assertEqual(len(lines), 6)
            for line in lines:
                digest, name = line.split('  ')
                self.assertEqual(digest, hashlib.sha256((dist / name).read_bytes()).hexdigest())
            write_checksums(dist)
            self.assertEqual((dist / 'SHA256SUMS').read_bytes(), checksum_bytes)

    def test_optional_proof_preserves_bytes_and_refuses_fixture_or_skipped_office(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'summary.json'
            dist = root / 'dist'
            dist.mkdir()
            self.assertFalse(copy_legion_proof(source, dist))
            valid = {'schemaVersion': 'ooxml-cli.legion-proof.v1', 'status': 'passed', 'skipOffice': False}
            for change in [{'fixtureOnly': True}, {'skipOffice': True}, {'status': 'failed'}, {'schemaVersion': 'unknown'}]:
                source.write_text(json.dumps(valid | change))
                with self.assertRaises(ValueError):
                    copy_legion_proof(source, dist)
                self.assertFalse((dist / 'legion-proof.json').exists())
            source.write_text(json.dumps(valid, indent=2))
            self.assertTrue(copy_legion_proof(source, dist))
            self.assertEqual(hashlib.sha256(source.read_bytes()).digest(),
                             hashlib.sha256((dist / 'legion-proof.json').read_bytes()).digest())


if __name__ == '__main__':
    unittest.main()
