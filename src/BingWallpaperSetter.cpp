#include "BingWallpaperSetter.h"
#include <QStandardPaths>
#include <QFile>
#include <QDateTime>
#include <QProcess>
#include <QDebug>
#include <QFileInfo>
#include <QSettings>
#include <QRegExp>

BingWallpaperSetter::BingWallpaperSetter(QObject *parent)
    : QObject(parent)
    , m_networkManager(new QNetworkAccessManager(this))
    , m_bingApiUrl("https://www.bing.com/HPImageArchive.aspx?format=js&idx=%1&n=1&mkt=zh-CN")
    , m_currentReply(nullptr)
    , m_isCustomDirectory(false)
    , m_currentOffset(0)
{
    loadSettings();
    setupWallpaperDirectory();
}

BingWallpaperSetter::~BingWallpaperSetter() {
    if (m_currentReply) {
        m_currentReply->abort();
    }
}

void BingWallpaperSetter::setupWallpaperDirectory() {
    QString picturesPath = QStandardPaths::writableLocation(QStandardPaths::PicturesLocation);
    m_defaultWallpaperDir = picturesPath + "/BingWallpapers";
    
    // 如果没有自定义路径，使用默认路径
    if (m_wallpaperDir.isEmpty()) {
        m_wallpaperDir = m_defaultWallpaperDir;
    }
    
    QDir dir;
    if (!dir.exists(m_wallpaperDir)) {
        dir.mkpath(m_wallpaperDir);
        qDebug() << "创建壁纸目录:" << m_wallpaperDir;
    }
}

void BingWallpaperSetter::loadSettings() {
    QSettings settings("BingWallpaper", "Settings");
    QString customDir = settings.value("wallpaperDirectory", "").toString();
    
    if (!customDir.isEmpty() && QDir(customDir).exists()) {
        m_wallpaperDir = customDir;
        m_isCustomDirectory = true;
        qDebug() << "加载自定义壁纸目录:" << m_wallpaperDir;
    }
}

void BingWallpaperSetter::saveSettings() {
    QSettings settings("BingWallpaper", "Settings");
    if (m_isCustomDirectory) {
        settings.setValue("wallpaperDirectory", m_wallpaperDir);
    } else {
        settings.remove("wallpaperDirectory");
    }
}

void BingWallpaperSetter::setWallpaperDirectory(const QString &directory) {
    if (directory.isEmpty() || !QDir(directory).exists()) {
        qDebug() << "无效的目录:" << directory;
        return;
    }
    
    m_wallpaperDir = directory;
    m_isCustomDirectory = (directory != m_defaultWallpaperDir);
    
    // 确保目录存在
    QDir dir;
    if (!dir.exists(m_wallpaperDir)) {
        dir.mkpath(m_wallpaperDir);
    }
    
    saveSettings();
    qDebug() << "壁纸目录已设置为:" << m_wallpaperDir;
}

bool BingWallpaperSetter::isCustomDirectory() const {
    return m_isCustomDirectory;
}

void BingWallpaperSetter::downloadAndSetWallpaper(int button) {
    emit downloadStarted();
    qDebug() << "正在获取Bing今日壁纸信息...";
    
    if (button == -1){
        m_currentOffset += 1;
    }else if (button == 1){
        m_currentOffset -= 1;
    }else{
        m_currentOffset = 0;
    }
    if (m_currentOffset < 0){
        m_currentOffset = 0;
    }else if (m_currentOffset > 7){
        m_currentOffset = 7;
    }
    
    QString url = m_bingApiUrl.arg(QString::number(m_currentOffset));
    QNetworkRequest request;
    request.setUrl(QUrl(url));
    request.setHeader(QNetworkRequest::UserAgentHeader, "Mozilla/5.0");
    
    m_currentReply = m_networkManager->get(request);
    connect(m_currentReply, &QNetworkReply::finished, this, &BingWallpaperSetter::onApiReplyFinished);
}

void BingWallpaperSetter::onApiReplyFinished() {
    if (!m_currentReply) return;
    
    if (m_currentReply->error() != QNetworkReply::NoError) {
        QString errorMsg = "API请求失败: " + m_currentReply->errorString();
        qDebug() << errorMsg;
        emit downloadFinished(false, errorMsg, m_currentOffset);
        m_currentReply->deleteLater();
        m_currentReply = nullptr;
        return;
    }
    
    QByteArray data = m_currentReply->readAll();
    m_currentReply->deleteLater();
    m_currentReply = nullptr;
    
    QJsonDocument doc = QJsonDocument::fromJson(data);
    if (doc.isNull() || !doc.isObject()) {
        emit downloadFinished(false, "解析API响应失败", m_currentOffset);
        return;
    }
    
    QJsonObject obj = doc.object();
    QJsonArray images = obj["images"].toArray();
    
    if (images.isEmpty()) {
        emit downloadFinished(false, "未找到壁纸信息", m_currentOffset);
        return;
    }
    
    QJsonObject imageInfo = images[0].toObject();
    QString imageUrl = "https://www.bing.com" + imageInfo["url"].toString();
    QString imageTitle = imageInfo["title"].toString();
    
    QStringList cr = imageInfo["copyright"].toString().split('(');
    QString imageCopyright = cr[0].replace(QChar(0xFF0C), '_').remove(' ');
    QString imageDate = imageInfo["startdate"].toString();
    
    // 替换为4K分辨率 (3840x2160)
    imageUrl.replace(QRegExp("\\d+x\\d+"), "UHD");
    
    qDebug() << "壁纸标题:" << imageTitle;
    qDebug() << "下载链接(4K):" << imageUrl;
    
    // 生成文件名
    //QString dateStr = QDateTime::currentDateTime().toString("yyyyMMdd");
    QString filename = QString("bing_wallpaper_%1_%2.jpg").arg(imageDate).arg(imageCopyright);
    m_currentWallpaperPath = m_wallpaperDir + "/" + filename;
    
    // 如果今天的壁纸已存在，直接使用
    if (QFile::exists(m_currentWallpaperPath)) {
        qDebug() << "今日壁纸已存在:" << m_currentWallpaperPath;
        if (setWallpaper(m_currentWallpaperPath)) {
            emit wallpaperSet(m_currentWallpaperPath);
            emit downloadFinished(true, "壁纸已设置（使用缓存）", m_currentOffset);
        } else {
            emit downloadFinished(false, "设置壁纸失败", m_currentOffset);
        }
        return;
    }
    
    // 下载壁纸
    qDebug() << "正在下载壁纸...";
    QNetworkRequest request;
    request.setUrl(QUrl(imageUrl));
    request.setHeader(QNetworkRequest::UserAgentHeader, "Mozilla/5.0");
    
    m_currentReply = m_networkManager->get(request);
    connect(m_currentReply, &QNetworkReply::finished, this, &BingWallpaperSetter::onImageDownloadFinished);
    connect(m_currentReply, &QNetworkReply::downloadProgress, this, &BingWallpaperSetter::onDownloadProgress);
}

void BingWallpaperSetter::onDownloadProgress(qint64 bytesReceived, qint64 bytesTotal) {
    if (bytesTotal > 0) {
        int percentage = (bytesReceived * 100) / bytesTotal;
        emit downloadProgress(percentage);
    }
}

void BingWallpaperSetter::onImageDownloadFinished() {
    if (!m_currentReply) return;
    
    if (m_currentReply->error() != QNetworkReply::NoError) {
        QString errorMsg = "壁纸下载失败: " + m_currentReply->errorString();
        qDebug() << errorMsg;
        emit downloadFinished(false, errorMsg, m_currentOffset);
        m_currentReply->deleteLater();
        m_currentReply = nullptr;
        return;
    }
    
    QByteArray imageData = m_currentReply->readAll();
    m_currentReply->deleteLater();
    m_currentReply = nullptr;
    
    qDebug() << "壁纸将保存到:" << m_currentWallpaperPath;
    // 保存壁纸
    QFile file(m_currentWallpaperPath);
    if (!file.open(QIODevice::WriteOnly)) {
        emit downloadFinished(false, "保存壁纸失败", m_currentOffset);
        return;
    }
    
    file.write(imageData);
    file.close();
    
    qDebug() << "壁纸已保存到:" << m_currentWallpaperPath;
    
    // 清理旧壁纸
    //cleanupOldWallpapers(7);
    
    // 设置壁纸
    if (setWallpaper(m_currentWallpaperPath)) {
        emit wallpaperSet(m_currentWallpaperPath);
        emit downloadFinished(true, "壁纸下载并设置成功！", m_currentOffset);
    } else {
        emit downloadFinished(false, "壁纸下载成功但设置失败", m_currentOffset);
    }
}

void BingWallpaperSetter::cleanupOldWallpapers(int keepDays) {
    QDir dir(m_wallpaperDir);
    QStringList filters;
    filters << "bing_wallpaper_*.jpg";
    QFileInfoList fileList = dir.entryInfoList(filters, QDir::Files, QDir::Name);
    
    if (fileList.size() > keepDays) {
        for (int i = 0; i < fileList.size() - keepDays; ++i) {
            QFile::remove(fileList[i].absoluteFilePath());
            qDebug() << "已删除旧壁纸:" << fileList[i].fileName();
        }
    }
}

QString BingWallpaperSetter::detectDesktopEnvironment() {
    QString desktop = qEnvironmentVariable("XDG_CURRENT_DESKTOP").toLower();
    
    if (desktop.contains("gnome") || desktop.contains("ubuntu")) {
        return "gnome";
    } else if (desktop.contains("kde") || desktop.contains("plasma")) {
        return "kde";
    }else if (desktop.contains("ukui")) {
        return "ukui";
    }
    
    return "unknown";
}

bool BingWallpaperSetter::setWallpaperGnome(const QString &imagePath) {
    QString fileUri = "file://" + imagePath;
    
    // 设置壁纸
    QProcess process;
    process.start("gsettings", QStringList() << "set" << "org.gnome.desktop.background" 
                  << "picture-uri" << fileUri);
    if (!process.waitForFinished(3000) || process.exitCode() != 0) {
        qDebug() << "设置picture-uri失败";
        return false;
    }
    
    // 设置暗色主题壁纸
    process.start("gsettings", QStringList() << "set" << "org.gnome.desktop.background" 
                  << "picture-uri-dark" << fileUri);
    if (!process.waitForFinished(3000) || process.exitCode() != 0) {
        qDebug() << "设置picture-uri-dark失败";
    }
    
    // 设置壁纸显示选项为缩放
    process.start("gsettings", QStringList() << "set" << "org.gnome.desktop.background" 
                  << "picture-options" << "zoom");
    if (!process.waitForFinished(3000) || process.exitCode() != 0) {
        qDebug() << "设置picture-options失败";
    }
    
    qDebug() << "GNOME壁纸设置成功";
    return true;
}

bool BingWallpaperSetter::setWallpaperUkui(const QString &imagePath) {
    //QString fileUri = "file://"imagePath;
    // 设置壁纸
    QProcess process;
    process.start("gsettings", QStringList() << "set" << "org.mate.background"
                  << "picture-filename" << imagePath);
    if (!process.waitForFinished(-1) || process.exitCode() != 0) {
        qDebug() << "设置picture-filename失败";
        return false;
    }
    // 设置壁纸显示选项为缩放
    process.start("gsettings", QStringList() << "set" << "org.mate.background"
                  << "picture-options" << "zoom");
    if (!process.waitForFinished(-1) || process.exitCode() != 0) {
        qDebug() << "设置picture-options失败";
    }
    qDebug() << "UKUI壁纸设置成功";
    return true;
}

bool BingWallpaperSetter::setWallpaperKde(const QString &imagePath) {
    QString script = QString(R"(
var allDesktops = desktops();
for (i=0; i<allDesktops.length; i++) {
    d = allDesktops[i];
    d.wallpaperPlugin = "org.kde.image";
    d.currentConfigGroup = Array("Wallpaper", "org.kde.image", "General");
    d.writeConfig("Image", "file://%1");
}
)").arg(imagePath);
    
    QProcess process;
    process.start("qdbus", QStringList() << "org.kde.plasmashell" << "/PlasmaShell"
                  << "org.kde.PlasmaShell.evaluateScript" << script);
    
    if (!process.waitForFinished(5000) || process.exitCode() != 0) {
        qDebug() << "设置KDE壁纸失败";
        return false;
    }
    
    qDebug() << "KDE壁纸设置成功";
    return true;
}

bool BingWallpaperSetter::setWallpaper(const QString &imagePath) {
    if (!QFile::exists(imagePath)) {
        qDebug() << "壁纸文件不存在:" << imagePath;
        return false;
    }
    
    QString desktop = detectDesktopEnvironment();
    qDebug() << "检测到桌面环境:" << desktop;
    
    if (desktop == "gnome") {
        return setWallpaperGnome(imagePath);
    } else if (desktop == "kde") {
        return setWallpaperKde(imagePath);
    } else if (desktop == "ukui") {
        return setWallpaperUkui(imagePath);
    } else {
        qDebug() << "未知桌面环境，尝试使用GNOME方法...";
        return setWallpaperGnome(imagePath);
    }
}

QString BingWallpaperSetter::getCurrentWallpaperPath() const {
    return m_currentWallpaperPath;
}

QString BingWallpaperSetter::getWallpaperDirectory() const {
    return m_wallpaperDir;
}
