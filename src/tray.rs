use crate::service::{Command, Event, Service};
use ksni::{blocking::TrayMethods, menu::StandardItem};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub struct Tray {
    pub service: Service,
    pub online: Arc<AtomicBool>,
}
impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "bing-wallpaper-rs".into()
    }
    fn title(&self) -> String {
        "Bing 壁纸 Rust".into()
    }
    fn icon_name(&self) -> String {
        "preferences-desktop-wallpaper".into()
    }
    fn activate(&mut self, _: i32, _: i32) {
        self.service.event(Event::Show);
    }
    fn watcher_offline(&self, _: ksni::OfflineReason) -> bool {
        self.online.store(false, Ordering::SeqCst);
        self.service.event(Event::Show);
        true
    }
    fn watcher_online(&self) {
        self.online.store(true, Ordering::SeqCst);
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            StandardItem {
                label: "打开主窗口".into(),
                activate: Box::new(|t: &mut Self| t.service.event(Event::Show)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "立即更新壁纸".into(),
                activate: Box::new(|t: &mut Self| t.service.update(false, false)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "打开壁纸文件夹".into(),
                activate: Box::new(|t: &mut Self| t.service.send(Command::OpenFolder)),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "退出".into(),
                activate: Box::new(|t: &mut Self| t.service.event(Event::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}
pub fn start(
    service: Service,
    online: Arc<AtomicBool>,
) -> Result<ksni::blocking::Handle<Tray>, ksni::Error> {
    let handle = Tray {
        service,
        online: online.clone(),
    }
    .spawn()?;
    online.store(true, Ordering::SeqCst);
    Ok(handle)
}
