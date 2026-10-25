mod app;
mod service;
mod tray;

use anyhow::{Context, Result};
use bing_wallpaper_rs::{
    backend::{self, Backend},
    bing::{self, State, Wallpaper},
    config::{self, Config},
    util,
};
use clap::{Parser, Subcommand};
use std::{path::PathBuf, sync::Arc};

#[derive(Parser)]
#[command(version, about = "Bing 每日壁纸 · Rust 原生桌面客户端")]
struct Cli {
    #[command(subcommand)]
    command: Option<Action>,
    /// Start in the tray if the desktop supports it.
    #[arg(long)]
    background: bool,
    /// Override desktop detection.
    #[arg(long, global = true, value_enum)]
    backend: Option<Backend>,
    /// Render a screenshot and exit (for desktop smoke tests).
    #[arg(long, hide = true)]
    smoke_screenshot: Option<PathBuf>,
}
#[derive(Subcommand)]
enum Action {
    /// Download and apply today's wallpaper.
    Update {
        #[arg(long)]
        force: bool,
    },
    /// Download without changing the desktop.
    Download {
        #[arg(long)]
        force: bool,
    },
    /// Apply an existing local image (also works offline).
    Set { path: PathBuf },
    /// Print desktop, session bus and tool diagnostics without changing settings.
    Doctor,
    /// Print the effective configuration, including imported Qt settings.
    Config,
    /// Enable or disable startup with the desktop session.
    Autostart {
        #[arg(value_parser = ["on", "off"])]
        mode: String,
    },
}

fn execute() -> Result<()> {
    let args = Cli::parse();
    if matches!(args.command, Some(Action::Doctor)) {
        println!("{}", backend::doctor(args.backend.unwrap_or(Backend::Auto)));
        println!("配置：{}", config::root()?.join("config.toml").display());
        println!(
            "错误日志：{}",
            config::state_root()?.join("errors.log").display()
        );
        return Ok(());
    }
    let mut config = Config::load()?;
    if let Some(backend) = args.backend {
        config.backend = backend;
    }
    match args.command {
        Some(Action::Update { force }) | Some(Action::Download { force }) => {
            let download_only = matches!(args.command, Some(Action::Download { .. }));
            let update = bing::update(&config, force, download_only, |p| eprintln!("{p}"))?;
            println!(
                "{}\n{}\n{}",
                update.wallpaper.title,
                update.wallpaper.path.display(),
                update.method
            );
            for warning in update.warnings {
                eprintln!("提示：{warning}");
            }
        }
        Some(Action::Set { path }) => {
            let _lock = util::lock(&config::state_root()?.join("update.lock"))?;
            let applied = backend::apply(&path, config.backend)?;
            println!("壁纸已设置：{}", applied.method);
            for warning in applied.warnings {
                eprintln!("提示：{warning}");
            }
            let state = State {
                wallpaper: Some(Wallpaper {
                    path: path.canonicalize()?,
                    title: "本地壁纸".into(),
                    copyright: String::new(),
                    date: String::new(),
                    market: config.market.clone(),
                }),
                last_success: Some(chrono::Utc::now().timestamp()),
                method: applied.method,
            };
            state.save().context("壁纸已设置，但状态保存失败")?;
        }
        Some(Action::Config) => println!("{}", toml::to_string_pretty(&config)?),
        Some(Action::Autostart { mode }) => {
            config::set_autostart(mode == "on")?;
            println!("登录启动已{}", if mode == "on" { "启用" } else { "关闭" });
        }
        Some(Action::Doctor) => unreachable!(),
        None => {
            if !config::root()?.join("config.toml").exists() {
                config.save()?;
            }
            let _instance = match util::lock(&config::state_root()?.join("gui.lock")) {
                Ok(lock) => lock,
                Err(e) => {
                    // Wake the existing instance without launching another updater.
                    util::atomic_write(&config::state_root()?.join("show-window"), b"show")?;
                    println!("已通知现有窗口：{e}");
                    return Ok(());
                }
            };
            let options = eframe::NativeOptions {
                viewport: eframe::egui::ViewportBuilder::default()
                    .with_title("Bing 壁纸 Rust")
                    .with_app_id("bing-wallpaper-rs")
                    .with_inner_size([1080.0, 820.0])
                    .with_min_inner_size([820.0, 640.0])
                    .with_icon(Arc::new(app::icon())),
                ..Default::default()
            };
            eframe::run_native(
                "bing-wallpaper-rs",
                options,
                Box::new(move |cc| {
                    Ok(Box::new(app::App::new(
                        cc,
                        config,
                        args.background,
                        args.smoke_screenshot,
                    )))
                }),
            )
            .map_err(|e| {
                anyhow::anyhow!(
                    "无法启动界面：{e}。请在图形会话中运行，或使用 update / doctor 命令"
                )
            })?;
        }
    }
    Ok(())
}
fn main() {
    if let Err(error) = execute() {
        eprintln!("错误：{error:#}");
        service::log_error(&format!("{error:#}"));
        std::process::exit(1);
    }
}
