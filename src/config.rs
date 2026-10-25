use crate::{backend::Backend, util};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub directory: PathBuf,
    pub auto_update: bool,
    pub interval_hours: u32,
    pub keep_count: usize,
    pub market: String,
    pub backend: Backend,
    pub close_to_tray: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            directory: dirs::picture_dir()
                .or_else(dirs::home_dir)
                .unwrap_or_else(|| PathBuf::from("."))
                .join("BingWallpapers"),
            auto_update: false,
            interval_hours: 24,
            keep_count: 7,
            market: "zh-CN".into(),
            backend: Backend::Auto,
            close_to_tray: true,
        }
    }
}

pub fn root() -> Result<PathBuf> {
    Ok(dirs::config_dir()
        .context("无法确定用户配置目录")?
        .join("bing-wallpaper-rs"))
}
pub fn state_root() -> Result<PathBuf> {
    Ok(dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .context("无法确定用户状态目录")?
        .join("bing-wallpaper-rs"))
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = root()?.join("config.toml");
        if path.exists() {
            let config: Self = toml::from_str(&fs::read_to_string(&path)?)
                .with_context(|| format!("配置格式错误：{}；请修正后重试", path.display()))?;
            config.validate()?;
            return Ok(config);
        }
        let mut config = Self::default();
        // Read only: leave the Qt application's settings intact.
        if let Some(dir) = dirs::config_dir()
            && let Ok(text) = fs::read_to_string(dir.join("BingWallpaper/Settings.conf"))
        {
            config.import_legacy(&text);
        }
        config.validate()?;
        Ok(config)
    }
    pub fn import_legacy(&mut self, text: &str) {
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "autoUpdate" => self.auto_update = value == "true",
                "updateInterval" => {
                    if let Ok(hours @ 1..=24) = value.parse() {
                        self.interval_hours = hours;
                    }
                }
                "wallpaperDirectory" => {
                    let path = PathBuf::from(value);
                    if path.is_absolute() && path.is_dir() {
                        self.directory = path;
                    }
                }
                _ => {}
            }
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(self.directory.is_absolute(), "壁纸目录必须是绝对路径");
        ensure!(
            (1..=24).contains(&self.interval_hours),
            "更新间隔应为 1–24 小时"
        );
        ensure!(
            (1..=365).contains(&self.keep_count),
            "保留数量应为 1–365 张"
        );
        let b = self.market.as_bytes();
        ensure!(
            b.len() == 5
                && b[2] == b'-'
                && b[..2].iter().all(u8::is_ascii_alphabetic)
                && b[3..].iter().all(u8::is_ascii_alphabetic),
            "地区格式应为 zh-CN、en-US 等"
        );
        Ok(())
    }
    pub fn save(&self) -> Result<()> {
        self.validate()?;
        util::atomic_write(
            &root()?.join("config.toml"),
            toml::to_string_pretty(self)?.as_bytes(),
        )
    }
}

pub fn autostart_path() -> Result<PathBuf> {
    Ok(dirs::config_dir()
        .context("无法确定用户配置目录")?
        .join("autostart/bing-wallpaper-rs.desktop"))
}

pub fn desktop_entry(executable: &Path, background: bool) -> String {
    let exec = executable
        .to_string_lossy()
        .replace('\\', "\\\\\\\\")
        .replace('"', "\\\\\"")
        .replace('`', "\\\\`")
        .replace('$', "\\\\$")
        .replace('%', "%%");
    format!(
        "[Desktop Entry]\nType=Application\nName=Bing Wallpaper Rust\nName[zh_CN]=Bing 壁纸 Rust\nComment=Bing daily wallpapers\nExec=\"{exec}\"{}\nIcon=bing-wallpaper-rs\nTerminal=false\nCategories=Utility;Settings;\nStartupWMClass=bing-wallpaper-rs\n",
        if background { " --background" } else { "" }
    )
}

pub fn set_autostart(enabled: bool) -> Result<()> {
    let path = autostart_path()?;
    if enabled {
        util::atomic_write(
            &path,
            desktop_entry(&std::env::current_exe()?, true).as_bytes(),
        )?;
    } else if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imports_old_settings_and_ignores_invalid_values() {
        let mut c = Config::default();
        c.import_legacy(
            "[General]\nautoUpdate=true\nupdateInterval=3\nwallpaperDirectory=/does/not/exist",
        );
        assert!(c.auto_update);
        assert_eq!(c.interval_hours, 3);
        c.import_legacy("updateInterval=0");
        assert_eq!(c.interval_hours, 3);
    }
    #[test]
    fn rejects_invalid_settings() {
        let mut c = Config {
            market: "../../tmp".into(),
            ..Config::default()
        };
        assert!(c.validate().is_err());
        c.market = "zh-CN".into();
        c.keep_count = 0;
        assert!(c.validate().is_err());
    }
    #[test]
    fn config_roundtrip() {
        let c = Config::default();
        let copy: Config = toml::from_str(&toml::to_string(&c).unwrap()).unwrap();
        assert_eq!(c.directory, copy.directory);
        assert_eq!(c.backend, copy.backend);
    }
}
