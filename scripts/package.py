#!/usr/bin/env python3
"""Build portable and Debian packages from a native Linux release binary."""
import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import tarfile
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parent.parent
DOCUMENTS = ['README.md', 'CONTRIBUTING.md', 'CHANGELOG.md', 'LICENSE']
PORTABLE_FILES = DOCUMENTS + [
    'scripts/install.py', 'scripts/uninstall.py',
    'assets/bing-wallpaper-rs.svg', 'assets/screenshot.png',
]


def binary_info(binary):
    data = binary.read_bytes()
    if len(data) < 64 or data[:6] != b'\x7fELF\x02\x01':
        raise ValueError('需要 64 位、小端 Linux ELF 二进制文件')
    machine = struct.unpack_from('<H', data, 18)[0]
    architectures = {62: ('x86_64', 'amd64'), 183: ('aarch64', 'arm64')}
    if machine not in architectures:
        raise ValueError(f'尚不支持 ELF 架构 {machine}')
    versions = re.findall(rb'GLIBC_(\d+\.\d+(?:\.\d+)?)\x00', data)
    if not versions:
        raise ValueError('无法确定 glibc 版本要求；当前打包脚本面向 glibc 构建')
    glibc = max(versions, key=lambda v: tuple(map(int, v.split(b'.')))).decode()
    archive_arch, deb_arch = architectures[machine]
    return archive_arch, deb_arch, glibc


def git_value(*args, default='unknown'):
    result = subprocess.run(['git', '-C', str(ROOT), *args], capture_output=True, text=True)
    return result.stdout.strip() if result.returncode == 0 else default


def archive_info(name, data, mode, epoch):
    info = tarfile.TarInfo(name)
    info.size = len(data)
    info.mode = mode
    info.mtime = epoch
    info.uid = info.gid = 0
    info.uname = info.gname = 'root'
    return info


def portable(binary, output, version, architecture, metadata, epoch):
    name = f'bing-wallpaper-rs-{version}-linux-{architecture}'
    path = output / f'{name}.tar.gz'
    with path.open('wb') as raw:
        with gzip.GzipFile(filename='', mode='wb', fileobj=raw, mtime=epoch) as compressed:
            with tarfile.open(fileobj=compressed, mode='w') as archive:
                files = [(f'{name}/bin/bing-wallpaper-rs', binary.read_bytes(), 0o755)]
                files += [(f'{name}/{file}', (ROOT / file).read_bytes(), 0o644) for file in PORTABLE_FILES]
                files += [(f'{name}/build-info.json', metadata, 0o644)]
                for entry, data, mode in files:
                    archive.addfile(archive_info(entry, data, mode, epoch), io.BytesIO(data))
    return path


def debian(binary, output, version, architecture, glibc, metadata, epoch):
    if not shutil.which('dpkg-deb'):
        raise ValueError('生成 DEB 需要 dpkg-deb；其他系统可使用 --format tar')
    path = output / f'bing-wallpaper-rs_{version}_{architecture}.deb'
    with tempfile.TemporaryDirectory(prefix='bing-deb-') as temporary:
        root = Path(temporary)

        def write(name, data, mode=0o644):
            target = root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data.encode() if isinstance(data, str) else data)
            target.chmod(mode)

        write('usr/bin/bing-wallpaper-rs', binary.read_bytes(), 0o755)
        write('usr/share/applications/bing-wallpaper-rs.desktop', '''[Desktop Entry]
Type=Application
Name=Bing Wallpaper Rust
Name[zh_CN]=Bing 壁纸 Rust
Comment=Bing daily wallpapers
Exec=/usr/bin/bing-wallpaper-rs
Icon=bing-wallpaper-rs
Terminal=false
Categories=Utility;
StartupWMClass=bing-wallpaper-rs
''')
        write('usr/share/icons/hicolor/scalable/apps/bing-wallpaper-rs.svg',
              (ROOT / 'assets/bing-wallpaper-rs.svg').read_bytes())
        for document in DOCUMENTS:
            write(f'usr/share/doc/bing-wallpaper-rs/{document}', (ROOT / document).read_bytes())
        for asset in ['screenshot.png', 'bing-wallpaper-rs.svg']:
            write(f'usr/share/doc/bing-wallpaper-rs/assets/{asset}', (ROOT / 'assets' / asset).read_bytes())
        write('usr/share/doc/bing-wallpaper-rs/copyright', (ROOT / 'LICENSE').read_bytes())
        write('usr/share/doc/bing-wallpaper-rs/build-info.json', metadata)
        size = sum(p.stat().st_size for p in root.rglob('*') if p.is_file())
        dependencies = [
            f'libc6 (>= {glibc})', 'libgcc-s1', 'libegl1', 'libgl1',
            'libx11-6', 'libxcb1', 'libxcursor1', 'libxi6', 'libxrandr2',
            'libxkbcommon0', 'libxkbcommon-x11-0',
            'libwayland-client0', 'libwayland-cursor0', 'libwayland-egl1',
        ]
        maintainer = 'xiaoff2004 <110325393+xiaoff2004@users.noreply.github.com>'
        write('DEBIAN/control', f'''Package: bing-wallpaper-rs
Version: {version}
Architecture: {architecture}
Maintainer: {maintainer}
Section: utils
Priority: optional
Installed-Size: {(size + 1023) // 1024}
Depends: {', '.join(dependencies)}
Recommends: fonts-noto-cjk, xdg-utils
Homepage: https://github.com/xiaoff2004/bing_wallpaper
Description: Bing daily wallpapers with a native Rust desktop interface
 Downloads and previews Bing wallpapers, supports background updates and
 integrates with KDE Plasma, GNOME and other Linux desktop environments.
 User wallpapers and settings are retained when the package is removed.
''')
        # No maintainer scripts: installing/removing never touches users' home directories.
        root.chmod(0o755)
        for directory in root.rglob('*'):
            if directory.is_dir():
                directory.chmod(0o755)
        environment = dict(os.environ, SOURCE_DATE_EPOCH=str(epoch))
        subprocess.run(['dpkg-deb', '--build', '--root-owner-group', '-Zxz', str(root), str(path)],
                       check=True, env=environment)
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/release/bing-wallpaper-rs')
    parser.add_argument('--output', type=Path, default=ROOT / 'dist')
    parser.add_argument('--format', choices=['all', 'tar', 'deb'], default='all')
    args = parser.parse_args()
    binary = args.binary.resolve()
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']['version']
    if not re.fullmatch(r'\d+\.\d+\.\d+', version):
        parser.error('当前发行脚本要求 x.y.z 形式的版本号')
    if not binary.is_file():
        parser.error('请先执行 cargo build --release --locked')
    try:
        archive_arch, deb_arch, glibc = binary_info(binary)
        actual = subprocess.check_output([str(binary), '--version'], text=True, timeout=10).strip()
        if actual != f'bing-wallpaper-rs {version}':
            parser.error(f'二进制版本与 Cargo.toml 不一致：{actual}')
        epoch = int(os.environ.get('SOURCE_DATE_EPOCH', git_value('show', '-s', '--format=%ct', default='0')))
        info = {
            'name': 'bing-wallpaper-rs', 'version': version, 'architecture': archive_arch,
            'debian_architecture': deb_arch, 'glibc_minimum': glibc,
            'source_commit': git_value('rev-parse', 'HEAD'),
            'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
            'source': 'https://github.com/xiaoff2004/bing_wallpaper',
        }
        metadata = (json.dumps(info, ensure_ascii=False, indent=2) + '\n').encode()
        output = args.output.resolve()
        output.mkdir(parents=True, exist_ok=True)
        artifacts = []
        if args.format in ['all', 'tar']:
            artifacts.append(portable(binary, output, version, archive_arch, metadata, epoch))
        if args.format in ['all', 'deb']:
            artifacts.append(debian(binary, output, version, deb_arch, glibc, metadata, epoch))
        info_path = output / 'build-info.json'
        info_path.write_bytes(metadata)
        artifacts.append(info_path)
        checksums = ''.join(f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n' for p in artifacts)
        (output / 'SHA256SUMS').write_text(checksums)
        for artifact in artifacts:
            print(artifact)
        print(output / 'SHA256SUMS')
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        parser.exit(1, f'打包失败：{error}\n')


if __name__ == '__main__':
    main()
