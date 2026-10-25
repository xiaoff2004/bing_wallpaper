# Bing 壁纸 Rust · 日景

这是 [xiaoff2004/bing_wallpaper](https://github.com/xiaoff2004/bing_wallpaper) 的 Rust 重写版，使用 **egui / eframe** 原生界面、**reqwest + rustls** 网络层、**zbus** 桌面通信和 **ksni** 系统托盘，沿用 MIT 许可证。

`main` 现使用 Rust 实现。原 Qt/C++ 版本仍保留在 [Git 历史](https://github.com/xiaoff2004/bing_wallpaper/tree/e0f6b48) 中；原来的 Qt 1.0.0 Release 不代表 Rust 版。Rust 客户端当前版本为 0.1.0。

本项目当前面向 **Linux 桌面**。不是 Windows/macOS 版本，也不是 Flatpak 沙箱应用。

![Rust 客户端界面](assets/screenshot.png)

## 下载安装包

从 [最新 Release](https://github.com/xiaoff2004/bing_wallpaper/releases/latest) 下载 Rust 版，当前自动构建提供 Linux x86_64 / amd64 包。

| 文件 | 用途 |
| --- | --- |
| `bing-wallpaper-rs_0.1.0_amd64.deb` | Ubuntu 24.04+ / 运行库满足要求的 Debian 系统安装包 |
| `bing-wallpaper-rs-0.1.0-linux-x86_64.tar.gz` | 解压运行或安装到用户目录 |
| `SHA256SUMS` | 下载校验 |
| `build-info.json` | 源提交、架构、最低 glibc 和二进制校验值 |

DEB 安装方式，在下载目录执行：

```bash
sudo apt install ./bing-wallpaper-rs_0.1.0_amd64.deb
```

便携版：

```bash
tar -xzf bing-wallpaper-rs-0.1.0-linux-x86_64.tar.gz
cd bing-wallpaper-rs-0.1.0-linux-x86_64
./bin/bing-wallpaper-rs
# 可选：安装到用户目录
python3 scripts/install.py
```

将全部发布附件放到同一目录后，可运行 `sha256sum --check SHA256SUMS` 验证。若之前用脚本安装 Rust 版，改用 DEB 前先退出程序并运行旧安装目录中的 `python3 scripts/uninstall.py`，避免用户级菜单和命令覆盖系统版本；个人壁纸与配置会保留。

## 功能

- 中文界面、深浅主题、图片预览、托盘更新和退出。
- 获取 Bing UHD 图片；UHD 不可用时尝试 1920×1080。实际尺寸由 Bing 提供。
- 可选择地区、保存目录、1–24 小时更新间隔和 1–365 张保留数量。
- 独立后台线程执行下载、定时更新与桌面操作，界面不被网络请求阻塞。
- HTTP 超时和有限重试；自动任务失败后 5 分钟重试；支持系统代理。
- 校验图片内容和尺寸，拒绝将 HTML 错误页或损坏文件当作壁纸；临时文件写完后原子替换。
- 使用 Bing 返回的日期命名文件；验证今日缓存后可离线应用，支持旧版 zh-CN 缓存。
- 错误信息保留后端名称、命令状态和 stderr，不再只显示“设置壁纸失败”。
- 清理只匹配程序生成的文件名，跳过符号链接，保护当前应用的图片。设置失败时不清理旧壁纸。
- 更新任务互斥，GUI 单实例；再次启动会唤起现有窗口。
- 首次读取原版 `~/.config/BingWallpaper/Settings.conf` 的更新间隔、自动更新开关和可用目录。新配置独立保存。

## 桌面兼容性

| 桌面 | 实现 | 说明 |
| --- | --- | --- |
| KDE Plasma 5 / 6 | 原生 D-Bus `evaluateScript`，设置所有桌面屏幕的图片插件 | 不依赖 `qdbus`；失败时依次尝试 `plasma-apply-wallpaperimage`、Qt 6/5 的 qdbus 工具 |
| GNOME / Unity | GSettings `picture-uri`，按实际键支持设置 `picture-uri-dark` | 旧版没有深色键也可用；检查配置是否被锁定 |
| Cinnamon | `org.cinnamon.desktop.background` | 使用桌面自身 schema |
| MATE | `org.mate.background` / `picture-filename` | 使用文件路径而不是 URI |
| XFCE | `xfconf-query` | 动态枚举现有显示器和工作区，需先在桌面设置中选过图片 |
| Sway | `swaymsg` | 检查 IPC 返回的 `success`，设置所有输出 |
| Hyprland / 其他 Wayland | `swww` | 需安装 swww 并运行 `swww-daemon`；不自动启动或替换用户的壁纸守护进程 |
| 其他 X11 窗口管理器 | 显式选择 `feh` | 不用于 Wayland；未知桌面不会盲目调用 GNOME 后端 |

当前实机验证以开发机器上的 KDE Wayland 为准。其他后端有隔离命令测试，仍需在对应桌面上实测。不支持的环境会返回明确错误；COSMIC、LXQt、Windows、macOS 和桌面 portal 尚未实现。

KDE 修改涵盖 Plasma `desktops()` 返回的桌面，包括不同活动。调用后确认配置已写入，但操作系统最终渲染仍由桌面管理器负责。

## 构建

需要 Rust 1.88 或更新版本。依赖版本已锁定在 `Cargo.lock`。

Ubuntu / Debian 的常规构建环境：

```bash
sudo apt install build-essential pkg-config libx11-dev libxi-dev libxrandr-dev libxkbcommon-dev libwayland-dev libgl1-mesa-dev fonts-noto-cjk
git clone https://github.com/xiaoff2004/bing_wallpaper.git
cd bing_wallpaper
cargo build --release --locked
```

应用不需要 Qt、WebKit 或 GTK 开发库。运行 GUI 需要桌面会话、OpenGL 驱动及 X11/Wayland 运行库；中文显示需要系统中文字体。HTTP 使用 rustls，无需 OpenSSL 开发库。

也可以运行 `sh scripts/build.sh`。该脚本优先使用系统 C 编译器；如果本机自行准备了 `.build-tools/cc` 和 `.build-tools/ar` 包装器，也可以使用用户级工具链。这些本机工具不随源码分发。

预编译包的运行库要求取决于构建环境。本次实测的 x86_64 构建需要 **glibc 2.39 或更新版本**（如 Ubuntu 24.04+）。在更旧的发行版上请从源码构建；桌面后端兼容与预编译包的运行库要求是两个独立条件。

如果 OpenGL 驱动有问题，可尝试系统提供的软件渲染：

```bash
LIBGL_ALWAYS_SOFTWARE=1 ./target/release/bing-wallpaper-rs
```

## 安装和运行

```bash
python3 scripts/install.py
~/.local/bin/bing-wallpaper-rs
```

用户级安装，不需要 sudo，不覆盖原版。应用菜单名称为 **Bing 壁纸 Rust**。请退出原版的后台程序，避免两个程序同时自动更新壁纸。

如果旧版通过 DEB 包安装，在确认 Rust 版正常运行后，可执行 `sudo apt remove bing-wallpaper-setter` 移除旧版。此操作保留个人壁纸和配置；无需使用旧版卸载脚本。

登录启动在“自动更新与设置”中开启，并点击“保存设置”。开启自动更新后，过期任务会在启动时补做。关闭到后台需要桌面支持系统托盘；Wayland 下采用最小化，X11 下隐藏窗口。没有托盘时关闭即退出，不会留下无法唤起的隐藏窗口。

```bash
# 下载并应用；有有效今日缓存时可离线使用
bing-wallpaper-rs update

# 忽略缓存重新获取
bing-wallpaper-rs update --force

# 仅下载；不修改桌面、不清理、不改变上次成功应用状态
bing-wallpaper-rs download

# 应用现有本地文件
bing-wallpaper-rs set '/path/to/wallpaper.jpg'

# 覆盖桌面识别（单次运行）
bing-wallpaper-rs --backend kde update

# 只读诊断；无需先启动 GUI
bing-wallpaper-rs doctor
bing-wallpaper-rs config

# 登录启动和后台启动
bing-wallpaper-rs autostart on
bing-wallpaper-rs autostart off
bing-wallpaper-rs --background
```

不要使用 sudo 启动应用。SSH、容器或系统级 cron 通常不具备桌面会话环境；请从桌面启动器或当前桌面终端运行。

## 配置和数据

- 配置：`$XDG_CONFIG_HOME/bing-wallpaper-rs/config.toml`（通常为 `~/.config/…`）。
- 状态和错误日志：`$XDG_STATE_HOME/bing-wallpaper-rs/`（通常为 `~/.local/state/…`）。
- 错误日志达到 512 KiB 后轮换一次，保留 `errors.previous.log`。
- 默认壁纸目录：XDG 图片目录下的 `BingWallpapers`，可在界面修改。
- 登录启动：`~/.config/autostart/bing-wallpaper-rs.desktop`。

已下载的图像版权归原权利人所有；预览展示 Bing 提供的标题和版权说明。旧版缓存没有元数据时显示通用标题，点击“重新获取”可补充信息。

原版目录路径若采用 Qt 特殊转义格式而未被识别，可在新界面手动填写；旧配置不会被改写。损坏的新配置会给出错误，不会静默覆盖。

## 验证

```bash
sh scripts/check.sh
```

测试包含桌面识别、URI/脚本转义、图片校验、HTTP 重试、清理保护、配置迁移、命令超时和隔离 CLI 后端测试。隔离测试使用临时 XDG 目录和模拟命令，不修改真实桌面。

可在图形会话运行一次截图冒烟检查：

```bash
./target/release/bing-wallpaper-rs --smoke-screenshot /tmp/bing-wallpaper.png
```

用户级脚本安装的卸载方式如下，删除二进制、菜单图标和登录启动项，保留壁纸及配置：

```bash
python3 scripts/uninstall.py
```

DEB 安装的版本使用 `sudo apt remove bing-wallpaper-rs` 卸载。用户自行启用的登录启动项，可在卸载前运行 `bing-wallpaper-rs autostart off` 关闭。

## 打包与自动发布

```bash
cargo build --release --locked
python3 scripts/package.py
python3 scripts/verify-package.py
```

默认生成 `.deb`、`.tar.gz`、`SHA256SUMS` 和 `build-info.json` 到 `dist/`，需要 Python 3.11+、`dpkg-deb`；校验脚本还需要 `desktop-file-validate`。非 Debian 构建环境可以使用 `python3 scripts/package.py --format tar` 仅生成便携包。

发布新版本时，更新 `Cargo.toml`、`Cargo.lock` 和更新日志，提交并推送代码，然后创建与 Cargo 版本一致的标签：

```bash
git tag -a v0.1.0 -m "Bing Wallpaper Rust 0.1.0"
git push origin refs/tags/v0.1.0
```

后续版本将示例中的 `0.1.0` 换成新版本号。`Release packages` 工作流会检查标签、执行 Rust 检查、构建并校验软件包，在 Ubuntu 24.04 runner 上安装和卸载 DEB，用虚拟显示检查 GUI 启动，最后公开 GitHub Release 并标记 Latest。上传失败会保留草稿，可重跑该标签的工作流；不会覆盖已经公开的发行版。`build-info.json` 记录构建来源，软件包校验不等于数字签名。

## 实现参考

- [eframe 0.33 文档](https://docs.rs/eframe/0.33.3/eframe/)
- [KDE Plasma 桌面脚本 API](https://develop.kde.org/docs/plasma/scripting/api/)
- [zbus](https://docs.rs/zbus/5.19.0/zbus/)
- [ksni 系统托盘](https://docs.rs/ksni/0.3.7/ksni/)

开发和提交说明见 [CONTRIBUTING.md](CONTRIBUTING.md)，版本变更见 [CHANGELOG.md](CHANGELOG.md)。
