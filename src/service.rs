use bing_wallpaper_rs::{
    backend,
    bing::{self, State, Update},
    config::Config,
    util,
};
use chrono::Utc;
use eframe::egui;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::Duration,
};

pub enum Command {
    Update { force: bool, download_only: bool },
    Configure(Config),
    Diagnose,
    OpenFolder,
    Quit,
}
pub enum Event {
    Progress(String),
    Finished(Result<Update, String>),
    Preview(PathBuf, Result<image::RgbaImage, String>),
    Diagnosis(String),
    Notice(String),
    Show,
    Quit,
}

#[derive(Clone)]
pub struct Service {
    commands: Sender<Command>,
    pub busy: Arc<AtomicBool>,
    pub events: Sender<Event>,
    context: egui::Context,
}
impl Service {
    pub fn update(&self, force: bool, download_only: bool) {
        if self
            .busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
            && self
                .commands
                .send(Command::Update {
                    force,
                    download_only,
                })
                .is_err()
        {
            self.busy.store(false, Ordering::SeqCst);
        }
        self.context.request_repaint();
    }
    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }
    pub fn event(&self, event: Event) {
        let _ = self.events.send(event);
        self.context.request_repaint();
    }
    pub fn preview(&self, path: PathBuf) {
        let service = self.clone();
        thread::spawn(move || {
            let image = bing::thumbnail(&path).map_err(|e| format!("{e:#}"));
            service.event(Event::Preview(path, image));
        });
    }
}

fn is_due(config: &Config, last_success: Option<i64>, retry_at: i64, now: i64) -> bool {
    config.auto_update
        && now >= retry_at
        && last_success
            .is_none_or(|last| now < last || now - last >= i64::from(config.interval_hours) * 3600)
}

pub fn start(config: Config, context: egui::Context) -> (Service, Receiver<Event>) {
    let (tx, rx) = mpsc::channel();
    let (events, receiver) = mpsc::channel();
    let service = Service {
        commands: tx,
        busy: Arc::new(AtomicBool::new(false)),
        events,
        context,
    };
    let worker = service.clone();
    thread::spawn(move || {
        let mut config = config;
        let mut last_success = State::load().ok().and_then(|s| s.last_success);
        let mut retry_at = 0;
        loop {
            if let Ok(root) = bing_wallpaper_rs::config::state_root() {
                let show = root.join("show-window");
                if show.exists() {
                    let _ = std::fs::remove_file(show);
                    worker.event(Event::Show);
                }
            }
            let command = match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(command) => command,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let now = Utc::now().timestamp();
                    if !is_due(&config, last_success, retry_at, now) {
                        continue;
                    }
                    if worker
                        .busy
                        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                        .is_err()
                    {
                        continue;
                    }
                    Command::Update {
                        force: false,
                        download_only: false,
                    }
                }
            };
            match command {
                Command::Quit => break,
                Command::Configure(new_config) => {
                    config = new_config;
                    retry_at = 0;
                }
                Command::Diagnose => {
                    worker.event(Event::Diagnosis(backend::doctor(config.backend)))
                }
                Command::OpenFolder => {
                    let result = std::fs::create_dir_all(&config.directory)
                        .map_err(anyhow::Error::from)
                        .and_then(|_| util::open_path(&config.directory));
                    if let Err(e) = result {
                        worker.event(Event::Notice(format!("无法打开文件夹：{e:#}")));
                    }
                }
                Command::Update {
                    force,
                    download_only,
                } => {
                    let result = bing::update(&config, force, download_only, |p| {
                        worker.event(Event::Progress(p))
                    });
                    match &result {
                        Ok(update) => {
                            if update.applied {
                                last_success = Some(Utc::now().timestamp());
                            }
                        }
                        Err(e) => {
                            retry_at = Utc::now().timestamp() + 300;
                            log_error(&format!("{e:#}"));
                        }
                    }
                    let preview = result.as_ref().ok().map(|u| u.wallpaper.path.clone());
                    worker.busy.store(false, Ordering::SeqCst);
                    worker.event(Event::Finished(result.map_err(|e| format!("{e:#}"))));
                    if let Some(path) = preview {
                        worker.preview(path);
                    }
                }
            }
        }
    });
    (service, receiver)
}

pub fn log_error(error: &str) {
    use std::io::Write;
    if let Ok(root) = bing_wallpaper_rs::config::state_root() {
        let _ = std::fs::create_dir_all(&root);
        let path = root.join("errors.log");
        if path.metadata().is_ok_and(|m| m.len() > 512 * 1024) {
            let _ = std::fs::rename(&path, root.join("errors.previous.log"));
        }
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(file, "{} {error}", chrono::Local::now().to_rfc3339());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scheduler_honors_interval_retry_and_disabled_state() {
        let mut config = Config {
            auto_update: true,
            interval_hours: 2,
            ..Config::default()
        };
        assert!(is_due(&config, None, 0, 10000));
        assert!(!is_due(&config, Some(10000), 0, 17199));
        assert!(is_due(&config, Some(10000), 0, 17200));
        assert!(!is_due(&config, None, 10300, 10299));
        assert!(is_due(&config, None, 10300, 10300));
        assert!(is_due(&config, Some(10000), 0, 9000));
        config.auto_update = false;
        assert!(!is_due(&config, None, 0, 10000));
    }
}
