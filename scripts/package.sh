#!/bin/bash

# Bing壁纸设置器 - 打包脚本
# 创建可分发的安装包

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_info() {
    echo -e "${BLUE}[信息]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[成功]${NC} $1"
}

print_error() {
    echo -e "${RED}[错误]${NC} $1"
}

print_header() {
    echo -e "${BLUE}================================================${NC}"
    echo -e "${BLUE}  Bing壁纸设置器 - 打包程序${NC}"
    echo -e "${BLUE}================================================${NC}"
    echo
}

# 检查依赖
check_dependencies() {
    print_info "检查打包依赖..."
    
    local missing_deps=()
    
    if ! command -v cmake &> /dev/null; then
        missing_deps+=("cmake")
    fi
    
    if ! command -v g++ &> /dev/null; then
        missing_deps+=("g++")
    fi
    
    if ! command -v qmake &> /dev/null && ! [ -f "/usr/lib/x86_64-linux-gnu/cmake/Qt5/Qt5Config.cmake" ]; then
        missing_deps+=("qtbase5-dev")
    fi
    
    if [ ${#missing_deps[@]} -ne 0 ]; then
        print_error "缺少依赖: ${missing_deps[*]}"
        print_info "请运行: sudo apt install cmake g++ qtbase5-dev qt5-qmake libqt5network5"
        exit 1
    fi
    
    print_success "所有依赖已安装"
}

# 编译程序
build_program() {
    print_info "开始编译程序..."
    
    SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
    PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
    cd "$PROJECT_ROOT"
    
    if [ -d "build" ]; then
        rm -rf build
    fi
    
    mkdir -p build
    cd build
    
    print_info "配置CMake (Release模式)..."
    cmake -DCMAKE_BUILD_TYPE=Release ../src || { print_error "CMake配置失败"; exit 1; }
    
    print_info "编译程序..."
    make -j$(nproc) || { print_error "编译失败"; exit 1; }
    
    print_success "编译成功"
    cd "$PROJECT_ROOT"
}

# 创建deb包结构
create_deb_package() {
    print_info "创建DEB安装包..."
    
    SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
    PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
    PKG_NAME="bing-wallpaper-setter"
    PKG_VERSION="1.1.0"
    PKG_ARCH="amd64"
    PKG_DIR="${PROJECT_ROOT}/dist/package/${PKG_NAME}_${PKG_VERSION}_${PKG_ARCH}"
    
    # 清理旧包
    rm -rf "${PROJECT_ROOT}/dist/package"
    mkdir -p "${PKG_DIR}"
    
    # 创建目录结构
    mkdir -p "${PKG_DIR}/usr/local/bin"
    mkdir -p "${PKG_DIR}/usr/share/applications"
    mkdir -p "${PKG_DIR}/usr/share/pixmaps"
    mkdir -p "${PKG_DIR}/usr/share/doc/${PKG_NAME}"
    mkdir -p "${PKG_DIR}/DEBIAN"
    
    # 复制可执行文件
    cp "${PROJECT_ROOT}/build/bin/BingWallpaperSetter" "${PKG_DIR}/usr/local/bin/"
    chmod +x "${PKG_DIR}/usr/local/bin/BingWallpaperSetter"
    
    # 复制desktop文件
    cp "${PROJECT_ROOT}/src/bing-wallpaper-setter.desktop" "${PKG_DIR}/usr/share/applications/"
    
    # 复制图标文件
    cp "${PROJECT_ROOT}/src/bing-wallpaper-setter.png" "${PKG_DIR}/usr/share/pixmaps/"
    
    # 复制文档
    cp "${PROJECT_ROOT}/README.md" "${PKG_DIR}/usr/share/doc/${PKG_NAME}/"
    
    # 创建control文件
    cat > "${PKG_DIR}/DEBIAN/control" << EOF
Package: ${PKG_NAME}
Version: ${PKG_VERSION}
Section: utils
Priority: optional
Architecture: ${PKG_ARCH}
Depends: libqt5core5a, libqt5gui5, libqt5widgets5, libqt5network5
Maintainer: BingWallpaper <admin@bingwallpaper.local>
Description: Bing每日4K壁纸自动设置器
 自动从Bing获取每日4K超高清壁纸并设置为桌面背景。
 .
 功能特点:
  - 图形界面操作
  - 4K超高清壁纸 (3840x2160)
  - 系统托盘支持
  - 自动更新功能
  - 壁纸预览
  - 自定义存储路径
EOF

    # 创建postinst脚本
    cat > "${PKG_DIR}/DEBIAN/postinst" << 'EOF'
#!/bin/bash
set -e

# 更新desktop数据库
if command -v update-desktop-database &> /dev/null; then
    update-desktop-database /usr/share/applications 2>/dev/null || true
fi

# 更新图标缓存
if command -v gtk-update-icon-cache &> /dev/null; then
    gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || true
fi

echo "Bing壁纸设置器安装完成！"
echo "运行方式："
echo "  1. 从应用程序菜单启动 'Bing壁纸设置器'"
echo "  2. 或在终端运行: BingWallpaperSetter"

exit 0
EOF

    chmod 755 "${PKG_DIR}/DEBIAN/postinst"
    
    # 创建postrm脚本
    cat > "${PKG_DIR}/DEBIAN/postrm" << 'EOF'
#!/bin/bash
set -e

if [ "$1" = "purge" ]; then
    # 清理配置文件
    rm -rf ~/.config/BingWallpaper 2>/dev/null || true
fi

# 更新desktop数据库
if command -v update-desktop-database &> /dev/null; then
    update-desktop-database /usr/share/applications 2>/dev/null || true
fi

# 更新图标缓存
if command -v gtk-update-icon-cache &> /dev/null; then
    gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || true
fi

exit 0
EOF

    chmod 755 "${PKG_DIR}/DEBIAN/postrm"
    
    # 构建deb包
    print_info "构建DEB包..."
    cd "${PROJECT_ROOT}/dist/package"
    dpkg-deb --build "${PKG_NAME}_${PKG_VERSION}_${PKG_ARCH}"
    
    # 移动deb包到dist目录并清理临时文件
    mv "${PKG_NAME}_${PKG_VERSION}_${PKG_ARCH}.deb" "${PROJECT_ROOT}/dist/"
    cd "${PROJECT_ROOT}"
    rm -rf "${PROJECT_ROOT}/dist/package"
    
    print_success "DEB包创建成功: dist/${PKG_NAME}_${PKG_VERSION}_${PKG_ARCH}.deb"
    
    cd "${PROJECT_ROOT}"
}

# 创建便携版
create_portable_version() {
    print_info "创建便携版..."
    
    SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
    PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
    PORTABLE_NAME="bing-wallpaper-setter"
    PORTABLE_DIR="${PROJECT_ROOT}/dist/${PORTABLE_NAME}"
    
    rm -rf "${PORTABLE_DIR}"
    mkdir -p "${PORTABLE_DIR}"
    
    # 复制可执行文件
    cp "${PROJECT_ROOT}/build/bin/BingWallpaperSetter" "${PORTABLE_DIR}/"
    cp "${PROJECT_ROOT}/src/bing-wallpaper-setter.desktop" "${PORTABLE_DIR}/"
    cp "${PROJECT_ROOT}/src/bing-wallpaper-setter.png" "${PORTABLE_DIR}/"
    cp "${PROJECT_ROOT}/README.md" "${PORTABLE_DIR}/"
    cp "${PROJECT_ROOT}/scripts/uninstall.sh" "${PORTABLE_DIR}/"
    
    chmod +x "${PORTABLE_DIR}/BingWallpaperSetter"
    
    # 创建安装脚本
    cat > "${PORTABLE_DIR}/install.sh" << 'EOF'
#!/bin/bash

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

echo "================================================"
echo "  Bing壁纸设置器 - 安装程序"
echo "================================================"
echo

# 复制可执行文件
echo "正在安装程序..."
sudo cp "$SCRIPT_DIR/BingWallpaperSetter" /usr/local/bin/
sudo chmod +x /usr/local/bin/BingWallpaperSetter

# 安装desktop文件
mkdir -p ~/.local/share/applications
cp "$SCRIPT_DIR/bing-wallpaper-setter.desktop" ~/.local/share/applications/
chmod +x ~/.local/share/applications/bing-wallpaper-setter.desktop

# 安装图标文件
sudo mkdir -p /usr/share/pixmaps
sudo cp "$SCRIPT_DIR/bing-wallpaper-setter.png" /usr/share/pixmaps/

# 更新desktop数据库
if command -v update-desktop-database &> /dev/null; then
    update-desktop-database ~/.local/share/applications 2>/dev/null || true
fi

echo
echo "✓ 安装完成！"
echo
echo "使用方法："
echo "  1. 从应用程序菜单启动 'Bing壁纸设置器'"
echo "  2. 或在终端运行: BingWallpaperSetter"
echo

read -p "是否现在启动程序？(y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    nohup BingWallpaperSetter > /dev/null 2>&1 &
    echo "程序已启动"
fi
EOF
    
    chmod +x "${PORTABLE_DIR}/install.sh"
    
    # 创建压缩包（包含文件夹）
    cd "${PROJECT_ROOT}/dist"
    tar -czf "bing-wallpaper-setter-portable.tar.gz" "${PORTABLE_NAME}"
    
    # 清理临时目录
    rm -rf "${PORTABLE_DIR}"
    
    print_success "便携版创建成功: dist/bing-wallpaper-setter-portable.tar.gz"
    cd "$PROJECT_ROOT"
}

# 显示打包信息
show_package_info() {
    echo
    echo -e "${GREEN}================================================${NC}"
    echo -e "${GREEN}  打包完成！${NC}"
    echo -e "${GREEN}================================================${NC}"
    echo
    print_info "生成的文件："
    echo
    echo "1. DEB安装包（推荐）:"
    echo "   dist/bing-wallpaper-setter_x.x.x_amd64.deb"
    echo "   安装方法: sudo dpkg -i dist/bing-wallpaper-setter_x.x.x_amd64.deb"
    echo "   或双击安装"
    echo
    echo "2. 便携版压缩包:"
    echo "   dist/bing-wallpaper-setter-portable.tar.gz"
    echo "   解压后运行 ./install.sh"
    echo
    print_info "系统要求："
    echo "   - Ubuntu 20.04+ 或其他Debian系Linux"
    echo "   - Qt5运行库 (通常已安装)"
    echo "   - GNOME/KDE/UKUI桌面环境"
    echo
}

# 主函数
main() {
    print_header
    
    # 检查依赖
    check_dependencies
    echo
    
    # 编译程序
    build_program
    echo
    
    # 创建deb包
    create_deb_package
    echo
    
    # 创建便携版
    create_portable_version
    
    # 显示信息
    show_package_info
}

# 运行主函数
main
