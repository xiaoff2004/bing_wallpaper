#include "MainWindow.h"
#include <QApplication>
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QGroupBox>
#include <QMessageBox>
#include <QCloseEvent>
#include <QDesktopServices>
#include <QUrl>
#include <QSettings>
#include <QPixmap>
#include <QLabel>
#include <QDialog>
#include <QScrollArea>
#include <QFileDialog>
#include <QPainter>
#include <QPainterPath>
#include <QDebug>
#include <QStyle>

MainWindow::MainWindow(QWidget *parent)
    : QMainWindow(parent)
    , m_wallpaperSetter(new BingWallpaperSetter(this))
    , m_trayIcon(new QSystemTrayIcon(this))
    , m_autoUpdateTimer(new QTimer(this))
    , m_isAutoUpdateEnabled(false)
    , m_updateIntervalHours(24)
{
    setupUI();
    setupSystemTray();
    loadSettings();
    
    // 连接信号
    connect(m_wallpaperSetter, &BingWallpaperSetter::downloadStarted, 
            this, &MainWindow::onDownloadStarted);
    connect(m_wallpaperSetter, &BingWallpaperSetter::downloadProgress, 
            this, &MainWindow::onDownloadProgress);
    connect(m_wallpaperSetter, &BingWallpaperSetter::downloadFinished, 
            this, &MainWindow::onDownloadFinished);
    connect(m_wallpaperSetter, &BingWallpaperSetter::wallpaperSet, 
            this, &MainWindow::onWallpaperSet);
    
    connect(m_autoUpdateTimer, &QTimer::timeout, this, &MainWindow::updateWallpaper);
    
    // 启动时更新一次壁纸
    QTimer::singleShot(1000, this, &MainWindow::updateWallpaper);
    
    // 启动时加载预览
    QTimer::singleShot(500, this, &MainWindow::updateWallpaperPreview);
}

MainWindow::~MainWindow() {
    saveSettings();
}

void MainWindow::setupUI() {
    setWindowTitle("Bing壁纸设置器");
    setMinimumSize(500, 400);
    setWindowIcon(createBingIcon(false));  // 窗口使用PNG
    
    QWidget *centralWidget = new QWidget(this);
    QVBoxLayout *mainLayout = new QVBoxLayout(centralWidget);
    
    // 状态组
    QGroupBox *statusGroup = new QGroupBox("状态信息", this);
    QVBoxLayout *statusLayout = new QVBoxLayout(statusGroup);
    
    m_statusLabel = new QLabel("准备就绪", this);
    m_statusLabel->setWordWrap(true);
    statusLayout->addWidget(m_statusLabel);
    
    m_currentWallpaperLabel = new QLabel("当前壁纸: 未设置", this);
    m_currentWallpaperLabel->setWordWrap(true);
    m_currentWallpaperLabel->setStyleSheet("QLabel { color: #555; font-size: 11px; }");
    statusLayout->addWidget(m_currentWallpaperLabel);
    
    m_progressBar = new QProgressBar(this);
    m_progressBar->setVisible(false);
    statusLayout->addWidget(m_progressBar);
    
    mainLayout->addWidget(statusGroup);
    
    // 壁纸预览组
    QGroupBox *previewGroup = new QGroupBox("🖼️ 当前壁纸预览", this);
    QVBoxLayout *previewLayout = new QVBoxLayout(previewGroup);
    
    m_wallpaperPreviewLabel = new QLabel(this);
    m_wallpaperPreviewLabel->setAlignment(Qt::AlignCenter);
    m_wallpaperPreviewLabel->setMinimumHeight(200);
    m_wallpaperPreviewLabel->setMaximumHeight(300);
    m_wallpaperPreviewLabel->setStyleSheet("QLabel { background: #f8f9fa; border: 2px solid #dee2e6; border-radius: 8px; }");
    m_wallpaperPreviewLabel->setScaledContents(false);
    m_wallpaperPreviewLabel->setText("🌄 暂无壁纸\n\n点击上方按钮更新壁纸");
    previewLayout->addWidget(m_wallpaperPreviewLabel);
    
    mainLayout->addWidget(previewGroup);
    
    // 操作组
    QGroupBox *actionGroup = new QGroupBox("操作", this);
    QVBoxLayout *actionLayout = new QVBoxLayout(actionGroup);
    
    QStyle* style = QApplication::style();
    
    // 创建水平布局放置三个按钮
    QHBoxLayout *buttonRowLayout = new QHBoxLayout();

    // 左侧按钮（“上一张”）
    m_prevButton = new QPushButton(QIcon(style->standardPixmap(QStyle::SP_ArrowLeft)), "上一张");
    connect(m_prevButton, &QPushButton::clicked, this, &MainWindow::onPrevWallpaper); 
    buttonRowLayout->addWidget(m_prevButton);

    // 中间的“立即更新壁纸”按钮（原 m_updateButton）
    m_updateButton = new QPushButton(QIcon(style->standardPixmap(QStyle::SP_BrowserReload)), "立即更新壁纸");
    //m_updateButton->setStyleSheet("QPushButton { padding: 10px; font-size: 14px; }");
    connect(m_updateButton, &QPushButton::clicked, this, &MainWindow::updateWallpaper);
    buttonRowLayout->addWidget(m_updateButton);

    // 右侧按钮（“下一张”）
    m_nextButton = new QPushButton(QIcon(style->standardPixmap(QStyle::SP_ArrowRight)), "下一张");
    connect(m_nextButton, &QPushButton::clicked, this, &MainWindow::onNextWallpaper); 
    buttonRowLayout->addWidget(m_nextButton);

    // 将包含三个按钮的水平布局添加到操作组的垂直布局中
    actionLayout->addLayout(buttonRowLayout);
    
    //m_updateButton = new QPushButton("🔄 立即更新壁纸", this);
    //m_updateButton = new QPushButton(QIcon(style->standardPixmap(QStyle::SP_BrowserReload)), "立即更新壁纸");
    
    //m_updateButton->setStyleSheet("QPushButton { padding: 10px; font-size: 14px; }");
    //connect(m_updateButton, &QPushButton::clicked, this, &MainWindow::updateWallpaper);
    //actionLayout->addWidget(m_updateButton);
    
    //m_openFolderButton = new QPushButton("📁 打开壁纸文件夹", this);
    m_openFolderButton = new QPushButton(QIcon(style->standardPixmap(QStyle::SP_DirOpenIcon)), "打开壁纸文件夹");
    connect(m_openFolderButton, &QPushButton::clicked, this, &MainWindow::openWallpaperFolder);
    actionLayout->addWidget(m_openFolderButton);
    mainLayout->addWidget(actionGroup);
    
    // 壁纸存储路径设置组
    QGroupBox *storageGroup = new QGroupBox("壁纸存储路径", this);
    QVBoxLayout *storageLayout = new QVBoxLayout(storageGroup);
    
    m_directoryLabel = new QLabel(this);
    m_directoryLabel->setWordWrap(true);
    m_directoryLabel->setStyleSheet("QLabel { color: #555; font-size: 11px; padding: 5px; background: #f0f0f0; border-radius: 3px; }");
    storageLayout->addWidget(m_directoryLabel);
    
    QHBoxLayout *directoryButtonLayout = new QHBoxLayout();
    
    m_changeDirectoryButton = new QPushButton("更改路径", this);
    connect(m_changeDirectoryButton, &QPushButton::clicked, this, &MainWindow::changeWallpaperDirectory);
    directoryButtonLayout->addWidget(m_changeDirectoryButton);
    
    m_resetDirectoryButton = new QPushButton("恢复默认", this);
    connect(m_resetDirectoryButton, &QPushButton::clicked, this, &MainWindow::resetWallpaperDirectory);
    directoryButtonLayout->addWidget(m_resetDirectoryButton);
    
    storageLayout->addLayout(directoryButtonLayout);
    mainLayout->addWidget(storageGroup);
    
    // 更新目录显示
    updateDirectoryLabel();
    
    // 自动更新组
    QGroupBox *autoUpdateGroup = new QGroupBox("自动更新设置", this);
    QVBoxLayout *autoUpdateLayout = new QVBoxLayout(autoUpdateGroup);
    
    m_autoUpdateCheckBox = new QCheckBox("启用自动更新", this);
    connect(m_autoUpdateCheckBox, &QCheckBox::toggled, this, &MainWindow::toggleAutoUpdate);
    autoUpdateLayout->addWidget(m_autoUpdateCheckBox);
    
    QHBoxLayout *intervalLayout = new QHBoxLayout();
    QLabel *intervalLabel = new QLabel("更新间隔:", this);
    intervalLayout->addWidget(intervalLabel);
    
    m_updateIntervalSpinBox = new QSpinBox(this);
    m_updateIntervalSpinBox->setRange(1, 24);
    m_updateIntervalSpinBox->setSuffix(" 小时");
    m_updateIntervalSpinBox->setValue(24);
    connect(m_updateIntervalSpinBox, QOverload<int>::of(&QSpinBox::valueChanged), [this](int value) {
        m_updateIntervalHours = value;
        if (m_isAutoUpdateEnabled) {
            m_autoUpdateTimer->start(m_updateIntervalHours * 3600000);
        }
        saveSettings();
    });
    intervalLayout->addWidget(m_updateIntervalSpinBox);
    intervalLayout->addStretch();
    
    autoUpdateLayout->addLayout(intervalLayout);
    mainLayout->addWidget(autoUpdateGroup);
    
    mainLayout->addStretch();
    
    // 关于信息
    QLabel *aboutLabel = new QLabel("Bing每日壁纸自动更新工具 v1.1", this);
    aboutLabel->setAlignment(Qt::AlignCenter);
    aboutLabel->setStyleSheet("QLabel { color: #888; font-size: 10px; margin-top: 10px; }");
    mainLayout->addWidget(aboutLabel);
    
    setCentralWidget(centralWidget);
}

QIcon MainWindow::createBingIcon(bool forTray) {
    // 如果是窗口图标，直接使用PNG文件
    if (!forTray) {
        QIcon icon("/usr/share/pixmaps/bing-wallpaper-setter.png");
        if (!icon.isNull()) {
            return icon;
        }
    }
    
    // 托盘图标：创建基于 Bing 官方 SVG 的图标
    QPixmap pixmap(64, 64);
    pixmap.fill(Qt::transparent);
    QPainter painter(&pixmap);
    painter.setRenderHint(QPainter::Antialiasing);
    
    // 托盘图标始终使用白色
    QColor iconColor(255, 255, 255);
    
    painter.setBrush(iconColor);
    painter.setPen(Qt::NoPen);
    
    // 缩放和偏移以适应64x64画布
    painter.translate(8, 8);
    painter.scale(2.0, 2.0);
    
    // 绘制 Bing SVG 路径
    QPainterPath path;
    path.moveTo(4.842, 0.005);
    
    // SVG path data 转换为 QPainterPath
    path.cubicTo(5.046, -0.042, 5.263, 0.005, 5.446, 0.147);
    path.lineTo(8.066, 1.960);
    path.cubicTo(8.435, 2.216, 8.558, 2.312, 8.703, 2.456);
    path.cubicTo(9.174, 2.926, 9.455, 3.546, 9.500, 4.221);
    path.lineTo(9.508, 5.068);
    path.lineTo(9.511, 6.509);
    path.lineTo(9.515, 19.511);
    path.lineTo(9.659, 19.417);
    path.lineTo(16.674, 15.064);
    path.lineTo(16.689, 15.067);
    path.lineTo(16.718, 15.077);
    path.cubicTo(16.320, 14.907, 15.825, 14.738, 15.063, 14.511);
    path.lineTo(14.579, 14.365);
    path.cubicTo(13.995, 14.185, 13.869, 14.127, 13.658, 13.985);
    path.cubicTo(13.525, 13.894, 13.405, 13.784, 13.288, 13.673);
    path.cubicTo(13.108, 13.488, 12.953, 13.272, 12.878, 13.081);
    path.lineTo(11.320, 9.063);
    path.cubicTo(11.154, 8.619, 11.154, 8.573, 11.164, 8.433);
    path.cubicTo(11.192, 8.153, 11.373, 7.918, 11.640, 7.805);
    path.cubicTo(11.754, 7.756, 11.883, 7.729, 11.970, 7.795);
    path.lineTo(12.094, 7.805);
    path.lineTo(12.146, 7.826);
    path.cubicTo(12.206, 7.852, 12.306, 7.901, 12.459, 7.980);
    path.lineTo(16.089, 9.888);
    path.cubicTo(17.452, 10.634, 18.574, 11.804, 19.381, 13.419);
    path.cubicTo(19.575, 14.409, 19.540, 15.456, 19.279, 16.431);
    path.cubicTo(19.063, 17.236, 18.640, 18.125, 18.225, 18.644);
    path.lineTo(18.145, 18.743);
    path.lineTo(18.098, 18.793);
    path.cubicTo(18.088, 18.803, 18.085, 18.803, 18.088, 18.795);
    path.lineTo(18.131, 18.721);
    path.lineTo(18.059, 18.835);
    path.cubicTo(18.048, 18.866, 17.826, 19.115, 17.679, 19.260);
    path.lineTo(17.509, 19.421);
    path.cubicTo(17.289, 19.623, 17.078, 19.781, 16.677, 20.041);
    path.lineTo(13.544, 23.0);
    path.cubicTo(12.603, 23.600, 11.684, 23.912, 10.631, 23.992);
    path.cubicTo(10.401, 24.010, 9.777, 24.000, 9.557, 23.975);
    path.cubicTo(9.035, 23.919, 8.534, 23.780, 7.899, 23.563);
    path.cubicTo(6.045, 22.825, 4.676, 21.275, 4.194, 19.368);
    path.cubicTo(4.126, 19.110, 4.083, 18.914, 4.073, 18.798);
    path.lineTo(4.027, 18.473);
    path.cubicTo(4.020, 18.395, 4.016, 18.327, 4.013, 18.305);
    path.lineTo(4.007, 18.276);
    path.lineTo(4.0, 11.617);
    path.lineTo(4.010, 0.866);
    path.cubicTo(4.011, 0.805, 4.014, 0.770, 4.017, 0.755);
    path.cubicTo(4.062, 0.464, 4.264, 0.218, 4.535, 0.116);
    path.cubicTo(4.647, 0.071, 4.751, 0.039, 4.842, 0.005);
    path.closeSubpath();
    
    painter.drawPath(path);
    
    return QIcon(pixmap);
}

void MainWindow::setupSystemTray() {
    // 设置托盘图标 - 使用动态生成的图标，适应主题
    m_trayIcon->setIcon(createBingIcon(true));
    m_trayIcon->setToolTip("Bing壁纸设置器");
    
    // 创建托盘菜单
    m_trayMenu = new QMenu(this);
    
    QAction *showAction = new QAction("显示主窗口", this);
    connect(showAction, &QAction::triggered, this, &QMainWindow::showNormal);
    m_trayMenu->addAction(showAction);
    
    QAction *prevAction = new QAction("上一张", this);
    connect(prevAction, &QAction::triggered, this, &MainWindow::onPrevWallpaper);
    m_trayMenu->addAction(prevAction);
    
    QAction *updateAction = new QAction("立即更新壁纸", this);
    connect(updateAction, &QAction::triggered, this, &MainWindow::updateWallpaper);
    m_trayMenu->addAction(updateAction);
    
    QAction *nextAction = new QAction("下一张", this);
    connect(nextAction, &QAction::triggered, this, &MainWindow::onNextWallpaper);
    m_trayMenu->addAction(nextAction);
    
    QAction *viewAction = new QAction("查看当前壁纸", this);
    connect(viewAction, &QAction::triggered, this, &MainWindow::viewCurrentWallpaper);
    m_trayMenu->addAction(viewAction);
    
    QAction *folderAction = new QAction("打开壁纸文件夹", this);
    connect(folderAction, &QAction::triggered, this, &MainWindow::openWallpaperFolder);
    m_trayMenu->addAction(folderAction);
    
    m_trayMenu->addSeparator();
    
    QAction *quitAction = new QAction("退出", this);
    connect(quitAction, &QAction::triggered, qApp, &QApplication::quit);
    m_trayMenu->addAction(quitAction);
    
    m_trayIcon->setContextMenu(m_trayMenu);
    
    connect(m_trayIcon, &QSystemTrayIcon::activated, 
            this, &MainWindow::trayIconActivated);
    
    m_trayIcon->show();
}

void MainWindow::loadSettings() {
    QSettings settings("BingWallpaper", "Settings");
    m_isAutoUpdateEnabled = settings.value("autoUpdate", false).toBool();
    m_updateIntervalHours = settings.value("updateInterval", 24).toInt();
    
    m_autoUpdateCheckBox->setChecked(m_isAutoUpdateEnabled);
    m_updateIntervalSpinBox->setValue(m_updateIntervalHours);
    
    if (m_isAutoUpdateEnabled) {
        m_autoUpdateTimer->start(m_updateIntervalHours * 3600000);
    }
}

void MainWindow::saveSettings() {
    QSettings settings("BingWallpaper", "Settings");
    settings.setValue("autoUpdate", m_isAutoUpdateEnabled);
    settings.setValue("updateInterval", m_updateIntervalHours);
}

void MainWindow::closeEvent(QCloseEvent *event) {
    if (m_trayIcon->isVisible()) {
        hide();
        event->ignore();
        
        /*if (m_isAutoUpdateEnabled) {
            m_trayIcon->showMessage("Bing壁纸设置器",
                                   "程序已最小化到系统托盘，将继续在后台运行",
                                   QSystemTrayIcon::Information, 3000);
        }*/
    }
}

void MainWindow::updateWallpaper() {
    m_updateButton->setEnabled(false);
    m_wallpaperSetter->downloadAndSetWallpaper(0);
}

void MainWindow::onPrevWallpaper() {
    m_prevButton->setEnabled(false);
    m_wallpaperSetter->downloadAndSetWallpaper(-1);
}

void MainWindow::onNextWallpaper() {
    m_nextButton->setEnabled(false);
    m_wallpaperSetter->downloadAndSetWallpaper(1);
}

void MainWindow::viewCurrentWallpaper() {
    QString wallpaperPath = m_wallpaperSetter->getCurrentWallpaperPath();
    
    if (wallpaperPath.isEmpty() || !QFile::exists(wallpaperPath)) {
        QMessageBox::information(this, "提示", "当前没有壁纸，请先更新壁纸");
        return;
    }
    
    // 创建对话框显示壁纸
    QDialog *dialog = new QDialog(this);
    dialog->setWindowTitle("当前壁纸预览");
    dialog->resize(800, 600);
    
    QVBoxLayout *layout = new QVBoxLayout(dialog);
    
    QScrollArea *scrollArea = new QScrollArea(dialog);
    scrollArea->setWidgetResizable(true);
    
    QLabel *imageLabel = new QLabel();
    QPixmap pixmap(wallpaperPath);
    if (!pixmap.isNull()) {
        imageLabel->setPixmap(pixmap.scaled(780, 580, Qt::KeepAspectRatio, Qt::SmoothTransformation));
        imageLabel->setAlignment(Qt::AlignCenter);
    } else {
        imageLabel->setText("无法加载壁纸图片");
    }
    
    scrollArea->setWidget(imageLabel);
    layout->addWidget(scrollArea);
    
    QLabel *pathLabel = new QLabel("路径: " + wallpaperPath);
    pathLabel->setWordWrap(true);
    pathLabel->setStyleSheet("QLabel { color: #555; font-size: 10px; }");
    layout->addWidget(pathLabel);
    
    dialog->exec();
    delete dialog;
}

void MainWindow::openWallpaperFolder() {
    QString folderPath = m_wallpaperSetter->getWallpaperDirectory();
    QDesktopServices::openUrl(QUrl::fromLocalFile(folderPath));
}

void MainWindow::changeWallpaperDirectory() {
    QString currentDir = m_wallpaperSetter->getWallpaperDirectory();
    QString newDir = QFileDialog::getExistingDirectory(this, 
                                                       "选择壁纸存储路径",
                                                       currentDir,
                                                       QFileDialog::ShowDirsOnly | QFileDialog::DontResolveSymlinks);
    
    if (!newDir.isEmpty() && newDir != currentDir) {
        m_wallpaperSetter->setWallpaperDirectory(newDir);
        updateDirectoryLabel();
        showStatusMessage("壁纸存储路径已更改为: " + newDir, 5000);
        
        QMessageBox::information(this, "路径已更改", 
                                QString("新的壁纸将保存到:\n%1\n\n旧壁纸仍保留在原位置。").arg(newDir));
    }
}

void MainWindow::resetWallpaperDirectory() {
    QMessageBox::StandardButton reply = QMessageBox::question(this, 
                                                              "恢复默认路径", 
                                                              "确定要恢复为默认壁纸存储路径吗？\n\n默认路径: ~/Pictures/BingWallpapers",
                                                              QMessageBox::Yes | QMessageBox::No);
    
    if (reply == QMessageBox::Yes) {
        QString picturesPath = QStandardPaths::writableLocation(QStandardPaths::PicturesLocation);
        QString defaultDir = picturesPath + "/BingWallpapers";
        m_wallpaperSetter->setWallpaperDirectory(defaultDir);
        updateDirectoryLabel();
        showStatusMessage("已恢复为默认壁纸存储路径", 3000);
    }
}

void MainWindow::updateDirectoryLabel() {
    QString dir = m_wallpaperSetter->getWallpaperDirectory();
    QString displayText = "当前路径: " + dir;
    
    if (m_wallpaperSetter->isCustomDirectory()) {
        displayText += " (自定义)";
        m_directoryLabel->setStyleSheet("QLabel { color: #2196F3; font-size: 11px; padding: 5px; background: #E3F2FD; border-radius: 3px; }");
    } else {
        displayText += " (默认)";
        m_directoryLabel->setStyleSheet("QLabel { color: #555; font-size: 11px; padding: 5px; background: #f0f0f0; border-radius: 3px; }");
    }
    
    m_directoryLabel->setText(displayText);
}

void MainWindow::updateWallpaperPreview() {
    QString wallpaperPath = m_wallpaperSetter->getCurrentWallpaperPath();
    
    if (wallpaperPath.isEmpty() || !QFile::exists(wallpaperPath)) {
        m_wallpaperPreviewLabel->setText("🌄 暂无壁纸\n\n点击上方按钮更新壁纸");
        m_wallpaperPreviewLabel->setPixmap(QPixmap());
        return;
    }
    
    QPixmap pixmap(wallpaperPath);
    if (!pixmap.isNull()) {
        // 缩放图片以适应预览区域
        int maxHeight = 280;
        QPixmap scaled = pixmap.scaledToHeight(maxHeight, Qt::SmoothTransformation);
        m_wallpaperPreviewLabel->setPixmap(scaled);
    } else {
        m_wallpaperPreviewLabel->setText("⚠️ 无法加载壁纸图片");
    }
}

void MainWindow::toggleAutoUpdate(bool enabled) {
    m_isAutoUpdateEnabled = enabled;
    
    if (enabled) {
        m_autoUpdateTimer->start(m_updateIntervalHours * 3600000);
        showStatusMessage("自动更新已启用，间隔: " + QString::number(m_updateIntervalHours) + " 小时");
    } else {
        m_autoUpdateTimer->stop();
        showStatusMessage("自动更新已禁用");
    }
    
    saveSettings();
}

void MainWindow::onDownloadStarted() {
    m_statusLabel->setText("正在下载壁纸...");
    m_progressBar->setValue(0);
    m_progressBar->setVisible(true);
}

void MainWindow::onDownloadProgress(int percentage) {
    m_progressBar->setValue(percentage);
}

void MainWindow::onDownloadFinished(bool success, const QString &message, int offset) {
    m_updateButton->setEnabled(true);
    m_prevButton->setEnabled(true);
    m_nextButton->setEnabled(true);
    if (offset == 0){
        m_nextButton->setEnabled(false);
    }
    if (offset == 7){
        m_prevButton->setEnabled(false);
    }
    m_progressBar->setVisible(false);
    
    if (success) {
        m_statusLabel->setText("✓ " + message);
        m_statusLabel->setStyleSheet("QLabel { color: green; }");
        //m_trayIcon->showMessage("成功", message, QSystemTrayIcon::Information, 3000);
    } else {
        m_statusLabel->setText("✗ " + message);
        m_statusLabel->setStyleSheet("QLabel { color: red; }");
        m_trayIcon->showMessage("错误", message, QSystemTrayIcon::Critical, 5000);
    }
    
    QTimer::singleShot(5000, [this]() {
        m_statusLabel->setStyleSheet("");
    });
}

void MainWindow::onWallpaperSet(const QString &path) {
    m_currentWallpaperLabel->setText("当前壁纸: " + path);
    updateWallpaperPreview();
}

void MainWindow::trayIconActivated(QSystemTrayIcon::ActivationReason reason) {
    if (reason == QSystemTrayIcon::Trigger || reason == QSystemTrayIcon::DoubleClick) {
        if (isVisible()) {
            hide();
        } else {
            showNormal();
            activateWindow();
        }
    }
}

void MainWindow::showStatusMessage(const QString &message, int timeout) {
    m_statusLabel->setText(message);
    QTimer::singleShot(timeout, [this]() {
        m_statusLabel->setText("准备就绪");
    });
}
