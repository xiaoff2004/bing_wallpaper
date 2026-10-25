# 贡献指南

当前 `main` 使用 Rust。旧版 Qt/C++ 实现可通过 Git 历史查看。

## 开发环境

按 [README](README.md#构建) 安装 Rust 和系统构建依赖，然后创建功能分支：

```bash
git switch -c feature/your-change
cargo build --locked
```

代码使用 `cargo fmt` 的格式规范。提交消息使用简明的 `feat:`、`fix:`、`docs:`、`test:` 或 `chore:` 前缀。

## 项目结构

- `src/app.rs`：中文界面、设置和预览。
- `src/service.rs`、`src/tray.rs`：后台调度和系统托盘。
- `src/bing.rs`：Bing API、下载、图片校验和缓存清理。
- `src/backend.rs`：桌面识别及各桌面壁纸后端。
- `src/config.rs`、`src/util.rs`：配置迁移、原子文件写入和命令执行。
- `src/main.rs`：命令行和 GUI 入口。
- `tests/backends.rs`：隔离环境下的命令行后端测试。

## 检查

提交前运行：

```bash
sh scripts/check.sh
```

该脚本执行格式检查、Clippy 和测试。自动测试使用临时 XDG 目录和模拟桌面工具，不修改真实桌面。为涉及后端的修改补充对应失败路径测试，特别注意路径转义、超时、权限锁定和部分成功的情况。

涉及 UI 或桌面接口的修改还应在实际桌面会话中检查。请记录测试过的发行版、桌面版本及 X11/Wayland 会话类型，区分实机测试与模拟测试。

## 报告问题和提交变更

通过 [Issues](https://github.com/xiaoff2004/bing_wallpaper/issues) 报告问题，提供复现步骤、应用版本和 `bing-wallpaper-rs doctor` 输出。附日志前检查其中的个人路径等信息。

PR 描述应说明问题、修改后的行为及验证结果。请保留 MIT 许可证，不提交 `target/`、`dist/`、用户配置、日志或本机工具链。`Cargo.lock` 随应用源码提交。

## 打包

```bash
cargo build --release --locked
python3 scripts/package.py
python3 scripts/verify-package.py
```

DEB、便携压缩包、构建信息及 SHA-256 文件输出到 `dist/`。打包脚本检查二进制版本并识别实际架构和 glibc 要求，避免错标软件包。

发布时将 Cargo 版本、锁文件和更新日志一并提交，推送对应 `vX.Y.Z` 标签。`Release packages` 工作流会构建和测试软件包，确认可安装、启动和卸载后发布 Release。普通分支提交只触发检查；发布流程不需要额外配置个人访问令牌，使用仓库自带的 `GITHUB_TOKEN`。已公开的 Release 不覆盖，修复后发布新版本。
