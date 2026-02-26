#ifndef BINGWALLPAPERSETTER_H
#define BINGWALLPAPERSETTER_H

#include <QObject>
#include <QString>
#include <QDir>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>

class BingWallpaperSetter : public QObject {
    Q_OBJECT

public:
    explicit BingWallpaperSetter(QObject *parent = nullptr);
    ~BingWallpaperSetter();
    
    void downloadAndSetWallpaper(int button);
    QString getCurrentWallpaperPath() const;
    QString getWallpaperDirectory() const;
    void setWallpaperDirectory(const QString &directory);
    bool isCustomDirectory() const;
    
signals:
    void downloadStarted();
    void downloadProgress(int percentage);
    void downloadFinished(bool success, const QString &message, int offset = 0);
    void wallpaperSet(const QString &path);
    
private slots:
    void onApiReplyFinished();
    void onImageDownloadFinished();
    void onDownloadProgress(qint64 bytesReceived, qint64 bytesTotal);
    
private:
    void setupWallpaperDirectory();
    void cleanupOldWallpapers(int keepDays = 7);
    bool setWallpaperGnome(const QString &imagePath);
    bool setWallpaperKde(const QString &imagePath);
    bool setWallpaperUkui(const QString &imagePath);
    QString detectDesktopEnvironment();
    bool setWallpaper(const QString &imagePath);
    void loadSettings();
    void saveSettings();
    
    QNetworkAccessManager *m_networkManager;
    QString m_wallpaperDir;
    QString m_defaultWallpaperDir;
    QString m_currentWallpaperPath;
    QString m_bingApiUrl;
    QNetworkReply *m_currentReply;
    bool m_isCustomDirectory;
    short m_currentOffset;
};

#endif // BINGWALLPAPERSETTER_H
