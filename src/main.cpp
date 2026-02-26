#include "MainWindow.h"
#include <QApplication>
#include <QStyleFactory>

int main(int argc, char *argv[]) {
    QApplication app(argc, argv);
    
    // 设置应用程序信息
    app.setApplicationName("Bing Wallpaper Setter");
    app.setApplicationDisplayName("Bing壁纸设置器");
    app.setOrganizationName("BingWallpaper");
    app.setOrganizationDomain("bingwallpaper.local");
    
    // 设置应用程序样式
    app.setStyle(QStyleFactory::create("Fusion"));
    
    // 确保应用程序不会在关闭所有窗口时退出（因为有系统托盘）
    app.setQuitOnLastWindowClosed(false);
    
    MainWindow window;
    //window.show();
    
    return app.exec();
}
