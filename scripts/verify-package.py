#!/usr/bin/env python3
"""Verify checksums, payloads and CLI startup without installing anything."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', nargs='?', type=Path, default=Path('dist'))
    args = parser.parse_args()
    directory = args.directory.resolve()
    metadata = json.loads((directory / 'build-info.json').read_text())
    checksums = (directory / 'SHA256SUMS').read_text().splitlines()
    assert len(checksums) == 3, 'Expected a portable package, a DEB and build metadata'
    assets = []
    for line in checksums:
        digest, name = line.split('  ', 1)
        assert Path(name).name == name, 'Unsafe checksum path'
        path = directory / name
        assert hashlib.sha256(path.read_bytes()).hexdigest() == digest, f'Checksum mismatch: {name}'
        assets.append(path)
    archive = next(p for p in assets if p.name.endswith('.tar.gz'))
    deb = next(p for p in assets if p.suffix == '.deb')

    def verify_binary(path):
        assert hashlib.sha256(path.read_bytes()).hexdigest() == metadata['binary_sha256']
        version = subprocess.check_output([str(path), '--version'], text=True, timeout=10).strip()
        assert version == 'bing-wallpaper-rs ' + metadata['version']

    with tempfile.TemporaryDirectory(prefix='bing-package-check-') as temporary:
        root = Path(temporary)
        with tarfile.open(archive) as source:
            for member in source.getmembers():
                assert member.isfile(), 'Portable payload must consist of regular files'
                assert not Path(member.name).is_absolute() and '..' not in Path(member.name).parts
                assert member.uid == 0 and member.gid == 0
            source.extractall(root / 'portable', filter='data')
        portable_root = next((root / 'portable').iterdir())
        verify_binary(portable_root / 'bin/bing-wallpaper-rs')
        for name in ['scripts/install.py', 'scripts/uninstall.py', 'README.md', 'LICENSE', 'assets/screenshot.png']:
            assert (portable_root / name).is_file(), name
        assert json.loads((portable_root / 'build-info.json').read_text()) == metadata
        for field, expected in [('Package', 'bing-wallpaper-rs'), ('Version', metadata['version']),
                                ('Architecture', metadata['debian_architecture'])]:
            value = subprocess.check_output(['dpkg-deb', '--field', str(deb), field], text=True).strip()
            assert value == expected, (field, value)
        dependencies = subprocess.check_output(['dpkg-deb', '--field', str(deb), 'Depends'], text=True)
        assert f"libc6 (>= {metadata['glibc_minimum']})" in dependencies
        payload = subprocess.check_output(['dpkg-deb', '--fsys-tarfile', str(deb)])
        with tarfile.open(fileobj=io.BytesIO(payload)) as contents:
            for entry in contents.getmembers():
                assert entry.uid == 0 and entry.gid == 0, entry.name
                if entry.isdir():
                    assert entry.mode == 0o755, (entry.name, oct(entry.mode))
        subprocess.run(['dpkg-deb', '--extract', str(deb), str(root / 'deb')], check=True)
        subprocess.run(['dpkg-deb', '--control', str(deb), str(root / 'control')], check=True)
        assert not any((root / 'control' / hook).exists() for hook in ['preinst', 'postinst', 'prerm', 'postrm'])
        verify_binary(root / 'deb/usr/bin/bing-wallpaper-rs')
        desktop = root / 'deb/usr/share/applications/bing-wallpaper-rs.desktop'
        subprocess.run(['desktop-file-validate', str(desktop)], check=True)
        assert 'Exec=/usr/bin/bing-wallpaper-rs\n' in desktop.read_text()
        assert not (root / 'deb/home').exists()
        assert json.loads((root / 'deb/usr/share/doc/bing-wallpaper-rs/build-info.json').read_text()) == metadata
    print('Package verification passed: checksums, DEB metadata, desktop entry, archive contents and CLI startup.')


if __name__ == '__main__':
    main()
