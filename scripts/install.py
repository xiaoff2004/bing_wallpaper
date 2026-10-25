#!/usr/bin/env python3
"""Install alongside the original Qt application, without root permissions."""
import argparse
import datetime
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent


def desktop_quote(value):
    return '"' + (str(value).replace('\\', '\\\\\\\\').replace('"', '\\\\"')
                  .replace('`', '\\\\`').replace('$', '\\\\$').replace('%', '%%')) + '"'


def atomic_copy(source, destination, executable=False):
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=destination.parent, delete=False) as file:
        temporary = Path(file.name)
    try:
        shutil.copyfile(source, temporary)
        temporary.chmod(0o755 if executable else 0o644)
        os.replace(temporary, destination)
    finally:
        temporary.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    default_binary = ROOT / 'target/release/bing-wallpaper-rs'
    if not default_binary.exists():
        default_binary = ROOT / 'bin/bing-wallpaper-rs'
    parser.add_argument('--binary', type=Path, default=default_binary)
    args = parser.parse_args()
    binary = args.binary.resolve()
    if not binary.is_file():
        parser.error(f'找不到构建产物：{binary}；请先运行 cargo build --release --locked')
    subprocess.run([str(binary), '--version'], check=True)
    data = Path(os.environ.get('XDG_DATA_HOME', Path.home() / '.local/share'))
    destination = Path.home() / '.local/bin/bing-wallpaper-rs'
    if destination.exists():
        backup = data / 'bing-wallpaper-rs/backups' / datetime.datetime.now().strftime('%Y%m%d-%H%M%S')
        backup.mkdir(parents=True, exist_ok=True)
        shutil.copy2(destination, backup / destination.name)
    atomic_copy(binary, destination, executable=True)
    atomic_copy(ROOT / 'assets/bing-wallpaper-rs.svg', data / 'icons/hicolor/scalable/apps/bing-wallpaper-rs.svg')
    application = data / 'applications/bing-wallpaper-rs.desktop'
    application.parent.mkdir(parents=True, exist_ok=True)
    application.write_text(
        '[Desktop Entry]\nType=Application\nName=Bing Wallpaper Rust\n'
        'Name[zh_CN]=Bing 壁纸 Rust\nComment=Bing daily wallpapers\n'
        f'Exec={desktop_quote(destination)}\nIcon=bing-wallpaper-rs\n'
        'Terminal=false\nCategories=Utility;Settings;\nStartupWMClass=bing-wallpaper-rs\n',
        encoding='utf-8')
    if shutil.which('update-desktop-database'):
        subprocess.run(['update-desktop-database', str(application.parent)], check=False)
    print(f'已安装：{destination}\n在应用菜单中搜索「Bing 壁纸 Rust」。')
    print('原版程序保持不变；首次启动时读取原版目录和更新间隔。登录启动请在新程序设置中开启。')


if __name__ == '__main__':
    main()
