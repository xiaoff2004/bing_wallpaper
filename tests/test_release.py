"""Check publication failure paths without connecting to GitHub."""
import importlib.util
import json
import os
from pathlib import Path
import runpy
import struct
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('package', ROOT / 'scripts/package.py')
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)


class PackageMetadata(unittest.TestCase):
    def test_architecture_and_numeric_glibc_versions(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'binary'
            elf = bytearray(64)
            elf[:6] = b'\x7fELF\x02\x01'
            struct.pack_into('<H', elf, 18, 62)
            path.write_bytes(elf + b'GLIBC_2.9\x00GLIBC_2.39\x00')
            self.assertEqual(package.binary_info(path), ('x86_64', 'amd64', '2.39'))
            struct.pack_into('<H', elf, 18, 183)
            path.write_bytes(elf + b'GLIBC_2.35\x00')
            self.assertEqual(package.binary_info(path), ('aarch64', 'arm64', '2.35'))

    def test_rejects_invalid_binary(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'binary'
            for data in [b'not ELF', b'\x7fELF\x02\x01']:
                path.write_bytes(data)
                with self.assertRaises(ValueError):
                    package.binary_info(path)


class ReleasePublication(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.previous_directory = Path.cwd()
        os.chdir(self.temporary.name)
        Path('dist').mkdir()
        self.info = {'name': 'bing-wallpaper-rs', 'version': '0.1.0',
                     'architecture': 'x86_64', 'debian_architecture': 'amd64',
                     'glibc_minimum': '2.39', 'source_commit': 'a' * 40}
        Path('dist/build-info.json').write_text(json.dumps(self.info))
        Path('dist/a.deb').write_bytes(b'deb')
        Path('dist/b.tar.gz').write_bytes(b'archive')
        Path('dist/SHA256SUMS').write_text('abc  a.deb\ndef  b.tar.gz\nghi  build-info.json\n')
        self.calls = []

    def tearDown(self):
        os.chdir(self.previous_directory)
        self.temporary.cleanup()

    def invoke(self, tag='v0.1.0', existing=None, upload_failure=False, commit=None):
        def command(args, **kwargs):
            self.calls.append(args)
            if args[:3] == ['gh', 'release', 'view']:
                return subprocess.CompletedProcess(args, 1 if existing is None else 0,
                                                   json.dumps({'isDraft': existing}))
            if args[:3] == ['gh', 'release', 'upload'] and upload_failure:
                raise subprocess.CalledProcessError(1, args)
            return subprocess.CompletedProcess(args, 0)

        with patch.dict(os.environ, {'RELEASE_TAG': tag}), \
             patch('subprocess.check_output', return_value=commit or 'a' * 40), \
             patch('subprocess.run', side_effect=command):
            runpy.run_path(str(ROOT / 'scripts/publish-release.py'), run_name='__main__')

    def test_tag_mismatch_stops_before_network(self):
        with self.assertRaises(AssertionError):
            self.invoke(tag='v0.2.0')
        self.assertEqual(self.calls, [])

    def test_wrong_source_commit_stops_before_network(self):
        with self.assertRaises(AssertionError):
            self.invoke(commit='b' * 40)
        self.assertEqual(self.calls, [])

    def test_public_release_is_not_overwritten(self):
        with self.assertRaises(AssertionError):
            self.invoke(existing=False)
        self.assertEqual(len(self.calls), 1)

    def test_upload_failure_does_not_publish_draft(self):
        with self.assertRaises(subprocess.CalledProcessError):
            self.invoke(upload_failure=True)
        self.assertFalse(any(c[:3] == ['gh', 'release', 'edit'] for c in self.calls))

    def test_existing_draft_can_resume(self):
        self.invoke(existing=True)
        self.assertEqual([c[2] for c in self.calls], ['view', 'upload', 'edit'])
        self.assertIn('--draft=false', self.calls[-1])


if __name__ == '__main__':
    unittest.main()
