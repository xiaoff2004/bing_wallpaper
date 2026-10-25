use crate::util;
use anyhow::{Context, Result, bail, ensure};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};

const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Backend {
    #[default]
    Auto,
    Kde,
    Gnome,
    Cinnamon,
    Mate,
    Xfce,
    Sway,
    Swww,
    Feh,
}
impl Backend {
    pub const ALL: [Self; 9] = [
        Self::Auto,
        Self::Kde,
        Self::Gnome,
        Self::Cinnamon,
        Self::Mate,
        Self::Xfce,
        Self::Sway,
        Self::Swww,
        Self::Feh,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "自动识别",
            Self::Kde => "KDE Plasma 5 / 6",
            Self::Gnome => "GNOME / Unity",
            Self::Cinnamon => "Cinnamon",
            Self::Mate => "MATE",
            Self::Xfce => "XFCE",
            Self::Sway => "Sway",
            Self::Swww => "swww（Wayland）",
            Self::Feh => "feh（X11）",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Applied {
    pub method: String,
    pub warnings: Vec<String>,
}

pub fn detect(desktop: &str, session: &str, sway_socket: bool, hyprland: bool) -> Result<Backend> {
    let d = format!("{desktop}:{session}").to_ascii_lowercase();
    // Specific desktops take precedence over distribution names such as "ubuntu".
    if d.contains("kde") || d.contains("plasma") {
        return Ok(Backend::Kde);
    }
    if d.contains("cinnamon") {
        return Ok(Backend::Cinnamon);
    }
    if d.contains("mate") {
        return Ok(Backend::Mate);
    }
    if d.contains("xfce") || d.contains("xubuntu") {
        return Ok(Backend::Xfce);
    }
    if d.contains("sway") || sway_socket {
        return Ok(Backend::Sway);
    }
    if d.contains("hyprland") || hyprland {
        return Ok(Backend::Swww);
    }
    if d.contains("gnome") || d.contains("unity") || d == "ubuntu:ubuntu" {
        return Ok(Backend::Gnome);
    }
    bail!(
        "无法识别桌面环境（{desktop} / {session}）。请在设置中指定后端，或运行 doctor 查看诊断；不会默认尝试 GNOME"
    )
}

pub fn selected(backend: Backend) -> Result<Backend> {
    if backend != Backend::Auto {
        return Ok(backend);
    }
    detect(
        &std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        &std::env::var("DESKTOP_SESSION").unwrap_or_default(),
        std::env::var_os("SWAYSOCK").is_some(),
        std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some(),
    )
}

pub fn session_bus() -> Result<zbus::blocking::Connection> {
    zbus::blocking::connection::Builder::session()?
        .method_timeout(TIMEOUT)
        .build()
        .context("无法连接桌面 D-Bus；请在已登录的桌面会话中，以当前用户运行（不要 sudo）")
}

pub fn kde_script(uri: &str) -> String {
    let quoted = serde_json::to_string(uri).expect("serializing a string cannot fail");
    format!(
        r#"var ds = desktops();
if (ds.length === 0) {{ throw new Error("No Plasma desktops available"); }}
for (var i = 0; i < ds.length; i++) {{
    var d = ds[i];
    d.wallpaperPlugin = "org.kde.image";
    d.currentConfigGroup = ["Wallpaper", "org.kde.image", "General"];
    d.writeConfig("Image", {quoted});
    if (d.readConfig("Image") !== {quoted}) {{ throw new Error("Wallpaper config did not persist"); }}
}}
print("BING_WALLPAPER_OK:" + ds.length);"#
    )
}

fn kde(path: &Path, uri: &str) -> Result<Applied> {
    let script = kde_script(uri);
    let direct = (|| -> Result<()> {
        let bus = session_bus()?;
        let proxy = zbus::blocking::Proxy::new(
            &bus,
            "org.kde.plasmashell",
            "/PlasmaShell",
            "org.kde.PlasmaShell",
        )?;
        let response: String = proxy.call("evaluateScript", &(script.as_str(),))?;
        ensure!(
            response.contains("BING_WALLPAPER_OK:"),
            "Plasma 未确认设置成功：{response}"
        );
        Ok(())
    })();
    match direct {
        Ok(()) => Ok(Applied {
            method: "KDE Plasma · 原生 D-Bus".into(),
            warnings: vec![],
        }),
        Err(e) => {
            let mut errors = vec![format!("原生 D-Bus：{e:#}")];
            if util::executable("plasma-apply-wallpaperimage").is_some() {
                match util::run(
                    "plasma-apply-wallpaperimage",
                    &[path.to_str().context("路径不是 UTF-8")?],
                    TIMEOUT,
                ) {
                    Ok(_) => {
                        return Ok(Applied {
                            method: "KDE · plasma-apply-wallpaperimage".into(),
                            warnings: errors,
                        });
                    }
                    Err(e) => errors.push(format!("{e:#}")),
                }
            }
            for command in [
                "qdbus6",
                "qdbus-qt6",
                "qdbus",
                "qdbus-qt5",
                "/usr/lib/qt6/bin/qdbus",
                "/usr/lib/qt5/bin/qdbus",
            ] {
                if util::executable(command).is_none() {
                    continue;
                }
                match util::run(
                    command,
                    &[
                        "org.kde.plasmashell",
                        "/PlasmaShell",
                        "org.kde.PlasmaShell.evaluateScript",
                        &script,
                    ],
                    TIMEOUT,
                ) {
                    Ok(out) if out.contains("BING_WALLPAPER_OK:") => {
                        return Ok(Applied {
                            method: format!("KDE · {command}"),
                            warnings: errors,
                        });
                    }
                    Ok(out) => errors.push(format!("{command} 未确认设置成功：{out}")),
                    Err(e) => errors.push(format!("{e:#}")),
                }
            }
            bail!(
                "KDE 壁纸设置失败。请确认 plasmashell 正在运行、桌面允许修改壁纸。\n{}",
                errors.join("\n")
            );
        }
    }
}

fn gvariant_string(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn gsettings(backend: Backend, path: &Path, uri: &str) -> Result<Applied> {
    let (schema, key, value) = match backend {
        Backend::Cinnamon => ("org.cinnamon.desktop.background", "picture-uri", uri),
        Backend::Mate => (
            "org.mate.background",
            "picture-filename",
            path.to_str().context("路径不是 UTF-8")?,
        ),
        _ => ("org.gnome.desktop.background", "picture-uri", uri),
    };
    let keys = util::run("gsettings", &["list-keys", schema], TIMEOUT)
        .context("桌面设置 schema 不可用，请确认选择了正确后端")?;
    ensure!(keys.lines().any(|k| k == key), "{schema} 不支持 {key}");
    let set = |key: &str, value: &str| -> Result<()> {
        let writable = util::run("gsettings", &["writable", schema, key], TIMEOUT)?;
        ensure!(writable == "true", "{schema}.{key} 被锁定，无法修改");
        util::run(
            "gsettings",
            &["set", schema, key, &gvariant_string(value)],
            TIMEOUT,
        )?;
        Ok(())
    };
    set(key, value)?;
    let mut warnings = Vec::new();
    if keys.lines().any(|k| k == "picture-uri-dark")
        && let Err(e) = set("picture-uri-dark", uri)
    {
        warnings.push(format!("浅色壁纸已设置，深色壁纸失败：{e:#}"));
    }
    if keys.lines().any(|k| k == "picture-options")
        && let Err(e) = set("picture-options", "zoom")
    {
        warnings.push(format!("缩放选项未设置：{e:#}"));
    }
    Ok(Applied {
        method: format!("{} · GSettings", backend.label()),
        warnings,
    })
}

pub fn xfce_properties(output: &str) -> Vec<&str> {
    output
        .lines()
        .map(str::trim)
        .filter(|p| {
            p.starts_with("/backdrop/")
                && (p.ends_with("/last-image") || p.ends_with("/image-path"))
        })
        .collect()
}

fn xfce(path: &Path) -> Result<Applied> {
    let output = util::run("xfconf-query", &["-c", "xfce4-desktop", "-l"], TIMEOUT)?;
    let props = xfce_properties(&output);
    ensure!(
        !props.is_empty(),
        "XFCE 尚未创建壁纸配置；请先在桌面设置中选择一张图片"
    );
    let mut errors = Vec::new();
    for prop in &props {
        if let Err(e) = util::run(
            "xfconf-query",
            &[
                "-c",
                "xfce4-desktop",
                "-p",
                prop,
                "-s",
                path.to_str().context("路径不是 UTF-8")?,
            ],
            TIMEOUT,
        ) {
            errors.push(format!("{prop}: {e:#}"));
        }
    }
    ensure!(
        errors.is_empty(),
        "XFCE 部分或全部桌面设置失败：\n{}",
        errors.join("\n")
    );
    Ok(Applied {
        method: format!("XFCE · {} 个壁纸配置", props.len()),
        warnings: vec![],
    })
}

pub fn apply(path: &Path, backend: Backend) -> Result<Applied> {
    let path = path.canonicalize().context("壁纸文件不存在")?;
    crate::bing::validate_image(&path)?;
    let uri = util::file_uri(&path)?;
    let backend = selected(backend)?;
    if backend == Backend::Kde {
        return kde(&path, &uri);
    }
    if matches!(backend, Backend::Gnome | Backend::Cinnamon | Backend::Mate) {
        return gsettings(backend, &path, &uri);
    }
    if backend == Backend::Xfce {
        return xfce(&path);
    }
    let path = path.to_str().context("路径不是 UTF-8")?;
    match backend {
        Backend::Sway => {
            let cmd = format!("output * bg {} fill", serde_json::to_string(path)?);
            let response = util::run("swaymsg", &["-t", "command", &cmd], TIMEOUT)?;
            let replies: Vec<serde_json::Value> = serde_json::from_str(&response)?;
            ensure!(
                !replies.is_empty() && replies.iter().all(|v| v["success"] == true),
                "Sway 拒绝了设置请求：{response}"
            );
        }
        Backend::Swww => {
            util::run("swww", &["query"], TIMEOUT)
                .context("需要安装 swww 并在 Wayland 会话中启动 swww-daemon")?;
            util::run("swww", &["img", path, "--transition-type", "fade"], TIMEOUT)?;
        }
        Backend::Feh => {
            ensure!(
                std::env::var("XDG_SESSION_TYPE").unwrap_or_default() != "wayland"
                    && std::env::var_os("WAYLAND_DISPLAY").is_none(),
                "feh 只能用于 X11，不能设置 Wayland 桌面壁纸"
            );
            util::run("feh", &["--bg-fill", "--", path], TIMEOUT)?;
        }
        _ => bail!("不支持的后端"),
    }
    Ok(Applied {
        method: backend.label().into(),
        warnings: vec![],
    })
}

pub fn doctor(backend: Backend) -> String {
    let mut lines = vec![format!("Bing Wallpaper Rust {}", env!("CARGO_PKG_VERSION"))];
    for key in [
        "XDG_CURRENT_DESKTOP",
        "DESKTOP_SESSION",
        "XDG_SESSION_TYPE",
        "DISPLAY",
        "WAYLAND_DISPLAY",
    ] {
        lines.push(format!(
            "{key} = {}",
            std::env::var(key).unwrap_or_else(|_| "（未设置）".into())
        ));
    }
    lines.push(match selected(backend) {
        Ok(b) => format!("壁纸后端 = {}", b.label()),
        Err(e) => format!("壁纸后端 = {e:#}"),
    });
    lines.push(match session_bus() {
        Ok(_) => "桌面 D-Bus = 已连接".into(),
        Err(e) => format!("桌面 D-Bus = {e:#}"),
    });
    for cmd in [
        "plasma-apply-wallpaperimage",
        "qdbus6",
        "qdbus",
        "gsettings",
        "xfconf-query",
        "swaymsg",
        "swww",
        "feh",
        "xdg-open",
    ] {
        lines.push(format!(
            "{cmd} = {}",
            util::executable(cmd)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "未安装".into())
        ));
    }
    lines.push("KDE 原生 D-Bus 后端不需要安装 qdbus。其他桌面只需要对应后端的工具。".into());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_distros_without_overriding_the_actual_desktop() {
        for (d, s, expected) in [
            ("ubuntu:KDE", "plasma", Backend::Kde),
            ("ubuntu:GNOME", "ubuntu", Backend::Gnome),
            ("X-Cinnamon", "cinnamon", Backend::Cinnamon),
            ("MATE", "mate", Backend::Mate),
            ("XFCE", "xubuntu", Backend::Xfce),
            ("sway", "", Backend::Sway),
            ("Hyprland", "", Backend::Swww),
        ] {
            assert_eq!(detect(d, s, false, false).unwrap(), expected);
        }
        assert!(detect("COSMIC", "cosmic", false, false).is_err());
        assert!(detect("", "", false, false).is_err());
    }
    #[test]
    fn script_escapes_paths_and_checks_result() {
        let uri = "file:///tmp/a\";danger();\n\\.jpg";
        let script = kde_script(uri);
        assert!(script.contains(&serde_json::to_string(uri).unwrap()));
        assert!(script.contains("BING_WALLPAPER_OK:"));
        assert!(script.contains("ds.length === 0"));
    }
    #[test]
    fn xfce_finds_real_monitors_and_workspaces() {
        let props = xfce_properties(
            "/backdrop/screen0/monitorDP-1/workspace0/last-image\n/backdrop/screen0/monitorHDMI-1/workspace2/last-image\n/backdrop/screen0/monitor0/image-path\n/backdrop/screen0/monitor0/image-style\n/unrelated/last-image",
        );
        assert_eq!(props.len(), 3);
    }
    #[test]
    fn gvariant_escapes_apostrophes_and_backslashes() {
        assert_eq!(gvariant_string("a'b\\c"), "'a\\'b\\\\c'");
    }
}
