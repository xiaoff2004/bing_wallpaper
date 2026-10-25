#!/usr/bin/env python3
"""Remove only the Rust application's launchers and binary. Keep pictures/settings."""
import os
from pathlib import Path

data = Path(os.environ.get('XDG_DATA_HOME', Path.home() / '.local/share'))
config = Path(os.environ.get('XDG_CONFIG_HOME', Path.home() / '.config'))
legacy = Path.home() / '.local/bin/BingWallpaperSetter'
if legacy.is_symlink() and legacy.resolve() == (Path.home() / '.local/bin/bing-wallpaper-rs').resolve():
    legacy.unlink()
    print(f'已移除兼容命令：{legacy}')
for path in [Path.home() / '.local/bin/bing-wallpaper-rs',
             data / 'applications/bing-wallpaper-rs.desktop',
             data / 'icons/hicolor/scalable/apps/bing-wallpaper-rs.svg',
             config / 'autostart/bing-wallpaper-rs.desktop']:
    path.unlink(missing_ok=True)
    print(f'已移除：{path}')
print('配置、历史日志和已下载的壁纸均已保留。已运行的程序请通过托盘菜单退出。')
