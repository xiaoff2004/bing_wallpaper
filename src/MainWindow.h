#ifndef MAINWINDOW_H
#define MAINWINDOW_H

#include <QMainWindow>
#include <QSystemTrayIcon>
#include <QMenu>
#include <QTimer>
#include <QLabel>
#include <QPushButton>
#include <QCheckBox>
#include <QSpinBox>
#include <QProgressBar>
#include "BingWallpaperSetter.h"

class MainWindow : public QMainWindow {
    Q_OBJECT

public:
    explicit MainWindow(QWidget *parent = nullptr);
    ~MainWindow();

protected:
    void closeEvent(QCloseEvent *event) override;

private slots:
    void onPrevWallpaper();
    void onNextWallpaper();
    void updateWallpaper();
    void viewCurrentWallpaper();
    void openWallpaperFolder();
    void changeWallpaperDirectory();
    void resetWallpaperDirectory();
    void toggleAutoUpdate(bool enabled);
    void onDownloadStarted();
    void onDownloadProgress(int percentage);
    void onDownloadFinished(bool success, const QString &message, int offset);
    void onWallpaperSet(const QString &path);
    void trayIconActivated(QSystemTrayIcon::ActivationReason reason);

private:
    void setupUI();
    void setupSystemTray();
    void loadSettings();
    void saveSettings();
    void showStatusMessage(const QString &message, int timeout = 3000);
    void updateDirectoryLabel();
    void updateWallpaperPreview();
    QIcon createBingIcon(bool forTray = false);

    BingWallpaperSetter *m_wallpaperSetter;
    QSystemTrayIcon *m_trayIcon;
    QMenu *m_trayMenu;
    QTimer *m_autoUpdateTimer;
    
    // UI组件
    QLabel *m_statusLabel;
    QLabel *m_currentWallpaperLabel;
    QLabel *m_wallpaperPreviewLabel;
    QLabel *m_directoryLabel;
    QPushButton *m_prevButton;
    QPushButton *m_nextButton;
    QPushButton *m_updateButton;
    QPushButton *m_openFolderButton;
    QPushButton *m_changeDirectoryButton;
    QPushButton *m_resetDirectoryButton;
    QCheckBox *m_autoUpdateCheckBox;
    QSpinBox *m_updateIntervalSpinBox;
    QProgressBar *m_progressBar;
    
    bool m_isAutoUpdateEnabled;
    int m_updateIntervalHours;
};

#endif // MAINWINDOW_H
