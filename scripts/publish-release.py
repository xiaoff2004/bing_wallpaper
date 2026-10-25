#!/usr/bin/env python3
"""Publish verified CI artifacts; a failed upload remains an unpublished draft."""
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

tag = os.environ['RELEASE_TAG']
info = json.loads(Path('dist/build-info.json').read_text())
assert re.fullmatch(r'v\d+\.\d+\.\d+', tag) and tag == 'v' + info['version']
commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
assert commit == info['source_commit'], 'Artifacts belong to a different commit'
assets = [Path('dist') / line.split('  ', 1)[1] for line in Path('dist/SHA256SUMS').read_text().splitlines()]
assert len(assets) == 3 and all(p.is_file() and p.parent == Path('dist') for p in assets)
assets.append(Path('dist/SHA256SUMS'))
version = info['version']
deb = f"bing-wallpaper-rs_{version}_{info['debian_architecture']}.deb"
archive = f"bing-wallpaper-rs-{version}-linux-{info['architecture']}.tar.gz"
notes = f'''Bing 壁纸 Rust {version}，提供中文原生界面、预览、托盘和后台自动更新。

### 下载与安装

- **Ubuntu 24.04+ / Debian（{info['debian_architecture']}）**：下载 `{deb}`，在下载目录运行 `sudo apt install ./{deb}`。
- **便携版**：下载 `{archive}`，解压后运行其中的 `bin/bing-wallpaper-rs`；也可用 `python3 scripts/install.py` 安装到用户目录。
- 本次构建需要 **glibc {info['glibc_minimum']}+**、图形桌面和 OpenGL 运行库。旧发行版请从源码构建。
- 将发布文件放在同一目录后，运行 `sha256sum --check SHA256SUMS` 验证下载。`build-info.json` 记录构建来源和二进制校验值。

### 从旧版迁移

退出旧版后运行新程序，可读取旧版配置和图片缓存。旧 Qt DEB 可通过 `sudo apt remove bing-wallpaper-setter` 卸载，个人壁纸和配置保留。

已通过用户级脚本安装 Rust 版、准备改用 DEB 的用户，应先退出应用并运行原安装目录中的 `python3 scripts/uninstall.py`，再安装 DEB，避免用户级入口覆盖系统版本。

### 验证与兼容性

- 发布前通过 Rust 格式检查、Clippy、自动测试及软件包解包启动检查。
- KDE 使用原生 D-Bus，解决 Qt 5/6 的 qdbus 命令兼容问题；KDE Wayland 已实机验证。
- GNOME、Cinnamon、MATE、XFCE、Sway/swww 后端有隔离测试，仍需对应桌面的实机验证。
- 这不是旧 Qt 1.0.0 安装包；旧 Release 保留供回退。

源代码提交：`{commit}`。
'''
existing = subprocess.run(['gh', 'release', 'view', tag, '--json', 'isDraft'], capture_output=True, text=True)
if existing.returncode == 0:
    assert json.loads(existing.stdout)['isDraft'], 'Release is already public; publish a new version instead of replacing its assets'
else:
    subprocess.run(['gh', 'release', 'create', tag, '--verify-tag', '--draft', '--title', f'Bing 壁纸 Rust {tag}'], check=True)
subprocess.run(['gh', 'release', 'upload', tag, *map(str, assets), '--clobber'], check=True)
with tempfile.NamedTemporaryFile(mode='w', encoding='utf-8', suffix='.md') as file:
    file.write(notes)
    file.flush()
    subprocess.run(['gh', 'release', 'edit', tag, '--title', f'Bing 壁纸 Rust {tag}',
                    '--notes-file', file.name, '--draft=false', '--latest'], check=True)
