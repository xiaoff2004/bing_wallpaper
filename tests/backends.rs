//! Run the real CLI with isolated XDG directories and fake desktop executables.
//! No test changes the user's actual desktop or process-wide environment.
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Output},
};

struct Desktop {
    root: tempfile::TempDir,
}
impl Desktop {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("bin")).unwrap();
        image::DynamicImage::new_rgb8(8, 8)
            .save(root.path().join("test '中文 #.png"))
            .unwrap();
        Self { root }
    }
    fn tool(&self, name: &str, script: &str) {
        let path = self.root.path().join("bin").join(name);
        fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fn run(&self, backend: &str, desktop: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_bing-wallpaper-rs"))
            .args(["--backend", backend, "set"])
            .arg(self.root.path().join("test '中文 #.png"))
            .env("PATH", self.root.path().join("bin"))
            .env("XDG_CONFIG_HOME", self.root.path().join("config"))
            .env("XDG_STATE_HOME", self.root.path().join("state"))
            .env("XDG_CURRENT_DESKTOP", desktop)
            .env("DESKTOP_SESSION", desktop)
            .env("XDG_SESSION_TYPE", "wayland")
            .env(
                "DBUS_SESSION_BUS_ADDRESS",
                "unix:path=/nonexistent-bing-test-bus",
            )
            .env_remove("SWAYSOCK")
            .env_remove("HYPRLAND_INSTANCE_SIGNATURE")
            .env("TEST_CALLS", self.root.path().join("calls"))
            .output()
            .unwrap()
    }
    fn calls(&self) -> String {
        fs::read_to_string(self.root.path().join("calls")).unwrap_or_default()
    }
}
fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn kde_falls_back_to_qdbus6_without_qdbus() {
    let desktop = Desktop::new();
    desktop.tool(
        "qdbus6",
        "printf '%s\\n' \"$@\" > \"$TEST_CALLS\"\nprintf 'BING_WALLPAPER_OK:2\\n'",
    );
    let result = desktop.run("auto", "ubuntu:KDE");
    assert_success(&result);
    assert!(String::from_utf8_lossy(&result.stdout).contains("qdbus6"));
    assert!(
        desktop
            .calls()
            .contains("org.kde.PlasmaShell.evaluateScript")
    );
    assert!(desktop.calls().contains("%23"));
}

#[test]
fn kde_prefers_native_wallpaper_tool_before_legacy_qdbus() {
    let desktop = Desktop::new();
    desktop.tool(
        "plasma-apply-wallpaperimage",
        "printf '%s\\n' \"$1\" > \"$TEST_CALLS\"",
    );
    desktop.tool("qdbus6", "exit 99");
    let output = desktop.run("kde", "KDE");
    assert_success(&output);
    assert!(Path::new(desktop.calls().trim()).exists());
    assert!(String::from_utf8_lossy(&output.stdout).contains("plasma-apply-wallpaperimage"));
}

#[test]
fn gnome_without_dark_key_still_succeeds() {
    let desktop = Desktop::new();
    desktop.tool(
        "gsettings",
        r#"
case "$1" in
 list-keys) printf 'picture-uri\npicture-options\n';;
 writable) echo true;;
 set) printf '%s\n' "$3" "$4" >> "$TEST_CALLS";;
 *) exit 90;;
esac
"#,
    );
    assert_success(&desktop.run("gnome", "GNOME"));
    assert!(!desktop.calls().contains("picture-uri-dark"));
    assert!(desktop.calls().contains("file://"));
    assert!(desktop.calls().contains("picture-options"));
}

#[test]
fn locked_gnome_setting_is_a_real_failure() {
    let desktop = Desktop::new();
    desktop.tool("gsettings", "case \"$1\" in list-keys) echo picture-uri;; writable) echo false;; set) echo SHOULD_NOT_RUN >> \"$TEST_CALLS\";; esac");
    let output = desktop.run("gnome", "GNOME");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("被锁定"));
    assert!(desktop.calls().is_empty());
}

#[test]
fn unknown_desktop_never_blindly_changes_gnome_settings() {
    let desktop = Desktop::new();
    desktop.tool("gsettings", "echo SHOULD_NOT_RUN >> \"$TEST_CALLS\"");
    let output = desktop.run("auto", "COSMIC");
    assert!(!output.status.success());
    assert!(desktop.calls().is_empty());
}

#[test]
fn sway_failure_payload_is_not_treated_as_success() {
    let desktop = Desktop::new();
    desktop.tool(
        "swaymsg",
        r#"echo '[{"success":false,"error":"no output"}]'"#,
    );
    let output = desktop.run("sway", "sway");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Sway 拒绝"));
}

#[test]
fn feh_is_rejected_on_wayland_before_execution() {
    let desktop = Desktop::new();
    desktop.tool("feh", "echo SHOULD_NOT_RUN >> \"$TEST_CALLS\"");
    assert!(!desktop.run("feh", "Hyprland").status.success());
    assert!(desktop.calls().is_empty());
}

#[test]
fn failures_include_tool_stderr() {
    let desktop = Desktop::new();
    desktop.tool("swww", "echo 'daemon socket is missing' >&2; exit 1");
    let output = desktop.run("swww", "Hyprland");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("daemon socket is missing"));
}

#[test]
fn mate_uses_filename_while_cinnamon_uses_uri() {
    for (backend, key, schema) in [
        ("mate", "picture-filename", "org.mate.background"),
        ("cinnamon", "picture-uri", "org.cinnamon.desktop.background"),
    ] {
        let desktop = Desktop::new();
        desktop.tool(
            "gsettings",
            &format!(
                r#"
case "$1" in
 list-keys) echo '{key}';;
 writable) echo true;;
 set) printf '%s\n' "$2" "$3" "$4" >> "$TEST_CALLS";;
esac
"#
            ),
        );
        assert_success(&desktop.run(backend, backend));
        let calls = desktop.calls();
        assert!(calls.contains(schema) && calls.contains(key));
        assert_eq!(calls.contains("file://"), backend == "cinnamon");
    }
}

#[test]
fn xfce_updates_each_detected_monitor_without_hardcoded_names() {
    let desktop = Desktop::new();
    desktop.tool("xfconf-query", r#"
if [ "$3" = '-l' ]; then
 printf '/backdrop/screen0/monitorDP-9/workspace0/last-image\n/backdrop/screen0/monitorHDMI-3/workspace2/last-image\n'
else
 printf '%s\n' "$4" "$6" >> "$TEST_CALLS"
fi
"#);
    assert_success(&desktop.run("xfce", "XFCE"));
    assert!(desktop.calls().contains("monitorDP-9"));
    assert!(desktop.calls().contains("monitorHDMI-3"));
    assert_eq!(desktop.calls().lines().count(), 4);
}

#[test]
fn optional_dark_theme_failure_is_reported_as_partial_success() {
    let desktop = Desktop::new();
    desktop.tool(
        "gsettings",
        r#"
case "$1" in
 list-keys) printf 'picture-uri\npicture-uri-dark\n';;
 writable) if [ "$3" = 'picture-uri-dark' ]; then echo false; else echo true; fi;;
 set) printf '%s\n' "$3" "$4" >> "$TEST_CALLS";;
esac
"#,
    );
    let output = desktop.run("gnome", "GNOME");
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("深色壁纸失败"));
}
