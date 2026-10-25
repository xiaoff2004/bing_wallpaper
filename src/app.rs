use crate::{
    service::{self, Command, Event, Service},
    tray,
};
use bing_wallpaper_rs::{
    backend::{self, Backend},
    bing::{self, State, Wallpaper},
    config::{self, Config},
};
use eframe::egui::{self, Color32, RichText};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::Receiver,
    },
    time::{Duration, Instant},
};

const ACCENT: Color32 = Color32::from_rgb(79, 191, 163);

pub struct App {
    config: Config,
    draft: Config,
    directory_text: String,
    autostart: bool,
    tab: usize,
    status: String,
    error: Option<String>,
    warnings: Vec<String>,
    wallpaper: Option<Wallpaper>,
    texture: Option<egui::TextureHandle>,
    method: String,
    diagnostics: String,
    service: Service,
    events: Receiver<Event>,
    tray: Option<ksni::blocking::Handle<tray::Tray>>,
    tray_online: Arc<AtomicBool>,
    quitting: bool,
    background: bool,
    started: Instant,
    smoke_screenshot: Option<PathBuf>,
    screenshot_requested: bool,
}

pub fn icon() -> egui::IconData {
    let mut rgba = Vec::with_capacity(64 * 64 * 4);
    for y in 0..64 {
        for x in 0..64 {
            let mountain = y > 46 - (x - 27_i32).abs() / 2 && y > 15;
            let sun = (x - 44_i32).pow(2) + (y - 19_i32).pow(2) < 49;
            rgba.extend_from_slice(if sun {
                &[244, 216, 144, 255]
            } else if mountain {
                &[79, 191, 163, 255]
            } else {
                &[21, 41, 46, 255]
            });
        }
    }
    egui::IconData {
        rgba,
        width: 64,
        height: 64,
    }
}

fn fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let paths = [
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJKsc-Regular.otf",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    ];
    // Fontconfig covers distributions with nonstandard font directories.
    let fallback = bing_wallpaper_rs::util::run(
        "fc-match",
        &["-f", "%{file}", "sans:lang=zh-cn"],
        Duration::from_secs(2),
    )
    .ok();
    for path in paths.into_iter().map(str::to_owned).chain(fallback) {
        if let Ok(bytes) = std::fs::read(&path) {
            let mut font = egui::FontData::from_owned(bytes);
            if path.ends_with("NotoSansCJK-Regular.ttc") {
                font.index = 2;
            }
            fonts.font_data.insert("chinese".into(), Arc::new(font));
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts
                    .families
                    .entry(family)
                    .or_default()
                    .push("chinese".into());
            }
            break;
        }
    }
    ctx.set_fonts(fonts);
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        config: Config,
        background: bool,
        smoke_screenshot: Option<PathBuf>,
    ) -> Self {
        fonts(&cc.egui_ctx);
        cc.egui_ctx.set_theme(egui::ThemePreference::System);
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(12.0, 12.0);
        style.spacing.button_padding = egui::vec2(16.0, 10.0);
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(16.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(16.0));
        cc.egui_ctx.set_style(style);
        let (service, events) = service::start(config.clone(), cc.egui_ctx.clone());
        let tray_online = Arc::new(AtomicBool::new(false));
        let (tray, tray_warning) = match tray::start(service.clone(), tray_online.clone()) {
            Ok(handle) => (Some(handle), None),
            Err(_) => (
                None,
                Some("当前桌面未提供系统托盘，关闭窗口将退出；保持窗口打开即可自动更新。".into()),
            ),
        };
        let (state, state_warning) = match State::load() {
            Ok(state) => (state, None),
            Err(e) => (State::default(), Some(format!("读取上次状态失败：{e:#}"))),
        };
        let wallpaper = state.wallpaper.filter(|w| w.path.exists()).or_else(|| {
            let mut paths: Vec<_> = std::fs::read_dir(&config.directory)
                .ok()?
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| bing::managed_date(p).is_some())
                .collect();
            paths.sort();
            let path = paths.pop()?;
            Some(Wallpaper {
                date: bing::managed_date(&path).unwrap_or_default(),
                path,
                title: "Bing 每日壁纸".into(),
                copyright: String::new(),
                market: config.market.clone(),
            })
        });
        if let Some(w) = &wallpaper {
            service.preview(w.path.clone());
        }
        Self {
            draft: config.clone(),
            directory_text: config.directory.display().to_string(),
            config,
            autostart: config::autostart_path().is_ok_and(|p| p.exists()),
            tab: 0,
            status: "准备就绪".into(),
            error: None,
            warnings: tray_warning.into_iter().chain(state_warning).collect(),
            wallpaper,
            texture: None,
            method: state.method,
            diagnostics: String::new(),
            service,
            events,
            tray,
            tray_online,
            quitting: false,
            background,
            started: Instant::now(),
            smoke_screenshot,
            screenshot_requested: false,
        }
    }

    fn show(&self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }
    fn hide(&self, ctx: &egui::Context) {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            // Wayland does not support winit's set_visible(false); minimize instead.
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
    }
    fn save_settings(&mut self) {
        let mut candidate = self.draft.clone();
        candidate.directory = PathBuf::from(self.directory_text.trim());
        match candidate.save() {
            Ok(()) => {
                self.config = candidate.clone();
                self.draft = candidate.clone();
                self.service.send(Command::Configure(candidate));
                self.status = "设置已保存".into();
                self.error = None;
                if let Err(e) = config::set_autostart(self.autostart) {
                    self.error = Some(format!("更新设置已保存，但登录启动设置失败：{e:#}"));
                }
            }
            Err(e) => self.error = Some(format!("设置未保存：{e:#}")),
        }
    }
    fn receive(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::Progress(message) => {
                    self.status = message;
                    self.error = None;
                }
                Event::Finished(Ok(update)) => {
                    self.wallpaper = Some(update.wallpaper);
                    self.method = update.method;
                    self.warnings = update.warnings;
                    self.status = if update.applied {
                        "桌面壁纸已更新"
                    } else {
                        "图片已下载，桌面壁纸未改变"
                    }
                    .into();
                    self.error = None;
                }
                Event::Finished(Err(error)) => {
                    self.status = "更新未完成".into();
                    self.error = Some(error);
                }
                Event::Preview(path, Ok(image)) => {
                    if !self.wallpaper.as_ref().is_some_and(|w| w.path == path) {
                        continue;
                    }
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        [image.width() as usize, image.height() as usize],
                        image.as_raw(),
                    );
                    self.texture = Some(ctx.load_texture(
                        "wallpaper-preview",
                        image,
                        egui::TextureOptions::LINEAR,
                    ));
                }
                Event::Preview(path, Err(e)) => {
                    if !self.wallpaper.as_ref().is_some_and(|w| w.path == path) {
                        continue;
                    }
                    self.texture = None;
                    self.warnings.push(format!("无法预览：{e}"));
                }
                Event::Diagnosis(report) => self.diagnostics = report,
                Event::Notice(message) => self.error = Some(message),
                Event::Show => self.show(ctx),
                Event::Quit => {
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }
    fn home(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("每天，换一个风景。").size(30.0).strong());
        ui.label(
            RichText::new("Bing 每日摄影 · 高清原图 · 自动更新")
                .color(ui.visuals().weak_text_color()),
        );
        ui.add_space(10.0);
        let width = ui.available_width();
        if let Some(texture) = &self.texture {
            ui.add(
                egui::Image::new(texture)
                    .fit_to_exact_size(egui::vec2(width, width * 0.5625))
                    .corner_radius(12),
            );
        } else {
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(width, width * 0.5625), egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, 12, ui.visuals().faint_bg_color);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                if self.wallpaper.is_some() {
                    "正在加载预览…"
                } else {
                    "更新今日壁纸，开始预览"
                },
                egui::FontId::proportional(16.0),
                ui.visuals().weak_text_color(),
            );
        }
        ui.add_space(4.0);
        if let Some(w) = &self.wallpaper {
            ui.label(
                RichText::new(if w.title.is_empty() {
                    "Bing 每日壁纸"
                } else {
                    &w.title
                })
                .size(21.0)
                .strong(),
            );
            if !w.copyright.is_empty() {
                ui.label(
                    RichText::new(&w.copyright)
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
            }
            ui.label(
                RichText::new(format!("{}  ·  {}", w.date, w.market))
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
        }
        ui.add_space(4.0);
        let busy = self.service.busy.load(Ordering::SeqCst);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !busy,
                    egui::Button::new(
                        RichText::new("立即更新壁纸")
                            .color(Color32::from_rgb(12, 36, 30))
                            .strong(),
                    )
                    .fill(ACCENT),
                )
                .clicked()
            {
                self.service.update(false, false);
            }
            if ui.add_enabled(!busy, egui::Button::new("仅下载")).clicked() {
                self.service.update(false, true);
            }
            if ui
                .add_enabled(!busy, egui::Button::new("重新获取"))
                .on_hover_text("忽略缓存，重新下载并设置今日壁纸")
                .clicked()
            {
                self.service.update(true, false);
            }
            if ui.button("打开文件夹").clicked() {
                self.service.send(Command::OpenFolder);
            }
        });
        if !self.method.is_empty() {
            ui.label(
                RichText::new(format!("上次操作：{}", self.method))
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
        }
    }
    fn settings(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("让风景如期而至").size(28.0).strong());
        ui.label("保存后生效。自动更新在程序运行期间执行，网络失败后 5 分钟重试。");
        ui.add_space(16.0);
        egui::Frame::group(ui.style())
            .inner_margin(20)
            .corner_radius(12)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.checkbox(&mut self.draft.auto_update, "启用自动更新");
                ui.horizontal(|ui| {
                    ui.label("检查间隔");
                    ui.add(
                        egui::Slider::new(&mut self.draft.interval_hours, 1..=24).suffix(" 小时"),
                    );
                });
                ui.checkbox(&mut self.autostart, "登录桌面时启动");
                ui.checkbox(
                    &mut self.draft.close_to_tray,
                    "关闭窗口时继续在后台运行（需要系统托盘）",
                );
                ui.label(
                    RichText::new("Wayland 下窗口会最小化；通过托盘菜单可重新打开或退出。")
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
            });
        ui.add_space(12.0);
        egui::Frame::group(ui.style())
            .inner_margin(20)
            .corner_radius(12)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("保存与来源").strong());
                ui.label("壁纸保存目录");
                ui.add(
                    egui::TextEdit::singleline(&mut self.directory_text)
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal(|ui| {
                    if ui.small_button("恢复默认目录").clicked() {
                        self.directory_text = Config::default().directory.display().to_string();
                    }
                    ui.label("保留最近");
                    ui.add(
                        egui::DragValue::new(&mut self.draft.keep_count)
                            .range(1..=365)
                            .suffix(" 张"),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Bing 地区");
                    egui::ComboBox::from_id_salt("market")
                        .selected_text(&self.draft.market)
                        .show_ui(ui, |ui| {
                            for market in ["zh-CN", "en-US", "ja-JP", "en-GB", "de-DE", "fr-FR"] {
                                ui.selectable_value(&mut self.draft.market, market.into(), market);
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("桌面后端");
                    egui::ComboBox::from_id_salt("backend")
                        .selected_text(self.draft.backend.label())
                        .show_ui(ui, |ui| {
                            for backend in Backend::ALL {
                                ui.selectable_value(
                                    &mut self.draft.backend,
                                    backend,
                                    backend.label(),
                                );
                            }
                        });
                });
            });
        ui.add_space(12.0);
        if ui.button(RichText::new("保存设置").strong()).clicked() {
            self.save_settings();
        }
    }
    fn diagnosis(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("兼容与诊断").size(28.0).strong());
        ui.label("检查当前桌面会话和工具，不修改壁纸。");
        ui.horizontal(|ui| {
            if ui.button("运行诊断").clicked() {
                self.diagnostics = "正在检查…".into();
                self.service.send(Command::Diagnose);
            }
            if ui.button("复制诊断结果").clicked() {
                ui.ctx().copy_text(self.diagnostics.clone());
            }
        });
        ui.add_space(10.0);
        ui.add(
            egui::TextEdit::multiline(&mut self.diagnostics)
                .desired_width(f32::INFINITY)
                .desired_rows(19)
                .interactive(false),
        );
        ui.label("KDE：直接连接 Plasma D-Bus；失败时再尝试系统工具。无需为 qdbus 创建软链接。");
        ui.label("GNOME / Cinnamon / MATE：GSettings；XFCE：动态枚举显示器；Sway / swww：调用对应桌面工具。");
        ui.label(
            RichText::new("其他桌面后端需要在对应系统中验证；不支持的环境会给出明确提示。")
                .small()
                .color(ui.visuals().weak_text_color()),
        );
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.receive(ctx);
        let tray_ready = self.tray.is_some() && self.tray_online.load(Ordering::SeqCst);
        if self.background {
            self.background = false;
            if tray_ready && self.smoke_screenshot.is_none() {
                self.hide(ctx);
            }
        }
        if ctx.input(|i| i.viewport().close_requested())
            && !self.quitting
            && self.config.close_to_tray
            && tray_ready
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide(ctx);
        }
        egui::SidePanel::left("navigation")
            .exact_width(220.0)
            .resizable(false)
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(24))
            .show(ctx, |ui| {
                ui.add_space(14.0);
                ui.label(RichText::new("日景").size(36.0).strong().color(ACCENT));
                ui.label(
                    RichText::new("BING WALLPAPER")
                        .size(11.0)
                        .extra_letter_spacing(1.5),
                );
                ui.add_space(38.0);
                for (index, label) in ["今日壁纸", "自动更新与设置", "兼容与诊断"]
                    .iter()
                    .enumerate()
                {
                    if ui
                        .add_sized(
                            [172.0, 44.0],
                            egui::Button::new(*label).selected(self.tab == index),
                        )
                        .clicked()
                    {
                        self.tab = index;
                    }
                }
                ui.add_space(26.0);
                ui.separator();
                ui.label(
                    RichText::new(
                        backend::selected(self.config.backend)
                            .map(|b| b.label())
                            .unwrap_or("待选择桌面后端"),
                    )
                    .small(),
                );
                ui.label(
                    RichText::new(if self.config.auto_update {
                        "自动更新已开启"
                    } else {
                        "手动更新"
                    })
                    .small()
                    .color(ACCENT),
                );
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.label(
                        RichText::new(concat!("Rust · v", env!("CARGO_PKG_VERSION")))
                            .small()
                            .color(ui.visuals().weak_text_color()),
                    );
                    if ui.small_button("退出程序").clicked() {
                        self.quitting = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    egui::widgets::global_theme_preference_switch(ui);
                });
            });
        egui::TopBottomPanel::bottom("status")
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(12))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if self.service.busy.load(Ordering::SeqCst) {
                        ui.spinner();
                    } else {
                        ui.label(RichText::new("●").color(if self.error.is_some() {
                            Color32::from_rgb(230, 132, 102)
                        } else {
                            ACCENT
                        }));
                    }
                    ui.label(&self.status);
                });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(30))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    match self.tab {
                        0 => self.home(ui),
                        1 => self.settings(ui),
                        _ => self.diagnosis(ui),
                    }
                    if let Some(error) = &self.error {
                        ui.add_space(12.0);
                        egui::Frame::group(ui.style())
                            .inner_margin(14)
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("操作未完成")
                                        .strong()
                                        .color(Color32::from_rgb(230, 132, 102)),
                                );
                                ui.label(error);
                                if ui.small_button("复制错误详情").clicked() {
                                    ctx.copy_text(error.clone());
                                }
                            });
                    }
                    for warning in &self.warnings {
                        ui.label(
                            RichText::new(warning)
                                .small()
                                .color(Color32::from_rgb(198, 148, 69)),
                        );
                    }
                });
            });
        if let Some(path) = &self.smoke_screenshot {
            if !self.screenshot_requested
                && self.started.elapsed() > Duration::from_secs(3)
                && (self.texture.is_some()
                    || self.wallpaper.is_none()
                    || self.started.elapsed() > Duration::from_secs(25))
            {
                self.screenshot_requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            for event in ctx.input(|i| i.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    let pixels: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                    match image::save_buffer(
                        path,
                        &pixels,
                        image.width() as u32,
                        image.height() as u32,
                        image::ColorType::Rgba8,
                    ) {
                        Ok(()) => println!("Screenshot: {}", path.display()),
                        Err(e) => eprintln!("Screenshot failed: {e}"),
                    }
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        ctx.request_repaint_after(Duration::from_secs(1));
    }
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        self.service.send(Command::Quit);
        if let Some(tray) = self.tray.take() {
            let _ = tray.shutdown();
        }
    }
}
