use crate::{
    backend,
    config::{self, Config},
    util,
};
use anyhow::{Context, Result, bail, ensure};
use chrono::{Local, NaiveDate, Utc};
use image::{DynamicImage, ImageReader, Limits};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

const MAX_IMAGE_BYTES: u64 = 30 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallpaper {
    pub path: PathBuf,
    pub title: String,
    pub copyright: String,
    pub date: String,
    pub market: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub wallpaper: Option<Wallpaper>,
    pub last_success: Option<i64>,
    pub method: String,
}
impl State {
    pub fn load() -> Result<Self> {
        let path = config::state_root()?.join("state.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        serde_json::from_slice(&fs::read(path)?).context("无法读取上次更新状态")
    }
    pub fn save(&self) -> Result<()> {
        util::atomic_write(
            &config::state_root()?.join("state.json"),
            &serde_json::to_vec_pretty(self)?,
        )
    }
}

#[derive(Debug)]
pub struct Update {
    pub wallpaper: Wallpaper,
    pub method: String,
    pub warnings: Vec<String>,
    pub applied: bool,
}

#[derive(Deserialize)]
struct ApiResponse {
    images: Vec<ApiImage>,
}
#[derive(Deserialize)]
struct ApiImage {
    urlbase: String,
    startdate: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    copyright: String,
}

pub fn decode_image(bytes: &[u8]) -> Result<DynamicImage> {
    ensure!(
        !bytes.is_empty() && bytes.len() as u64 <= MAX_IMAGE_BYTES,
        "图片为空或超过 30 MiB"
    );
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().context("图片内容损坏或格式不支持")?;
    ensure!(image.width() > 0 && image.height() > 0, "图片尺寸无效");
    Ok(image)
}
pub fn validate_image(path: &Path) -> Result<()> {
    let file = fs::File::open(path)?;
    ensure!(file.metadata()?.len() <= MAX_IMAGE_BYTES, "图片超过 30 MiB");
    let mut bytes = Vec::new();
    file.take(MAX_IMAGE_BYTES + 1).read_to_end(&mut bytes)?;
    decode_image(&bytes)?;
    Ok(())
}
pub fn thumbnail(path: &Path) -> Result<image::RgbaImage> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    Ok(decode_image(&bytes)?.thumbnail(1400, 900).to_rgba8())
}

pub fn valid_date(date: &str) -> bool {
    date.len() == 8
        && date.bytes().all(|b| b.is_ascii_digit())
        && NaiveDate::parse_from_str(date, "%Y%m%d").is_ok()
}

fn image_url(base: &str, resolution: &str) -> Result<url::Url> {
    ensure!(
        base.starts_with("/th?") && !base.contains('#'),
        "Bing 返回了意外的图片地址"
    );
    let url = url::Url::parse(&format!("https://www.bing.com{base}_{resolution}.jpg"))?;
    ensure!(
        url.host_str() == Some("www.bing.com"),
        "图片地址不属于 Bing"
    );
    Ok(url)
}

pub fn fetch_bytes(client: &Client, url: &str, limit: u64) -> Result<Vec<u8>> {
    let mut last = String::new();
    for attempt in 0..3 {
        let result = (|| -> Result<Vec<u8>> {
            let response = client.get(url).send()?.error_for_status()?;
            ensure!(
                response.content_length().is_none_or(|n| n <= limit),
                "响应内容超过大小限制"
            );
            let mut bytes = Vec::new();
            response.take(limit + 1).read_to_end(&mut bytes)?;
            ensure!(bytes.len() as u64 <= limit, "响应内容超过大小限制");
            Ok(bytes)
        })();
        match result {
            Ok(bytes) => return Ok(bytes),
            Err(e) => last = format!("{e:#}"),
        }
        if attempt < 2 {
            thread::sleep(Duration::from_millis(300 * (1 << attempt)));
        }
    }
    bail!("请求失败（已尝试 3 次）：{last}")
}

fn cached(config: &Config, date: &str) -> Option<Wallpaper> {
    let paths = [
        config
            .directory
            .join(format!("bing_wallpaper_{date}_{}.jpg", config.market)),
        config.directory.join(format!("bing_wallpaper_{date}.jpg")),
    ];
    for path in paths {
        if validate_image(&path).is_err() {
            continue;
        }
        if let Ok(bytes) = fs::read(path.with_extension("json"))
            && let Ok(mut entry) = serde_json::from_slice::<Wallpaper>(&bytes)
            && entry.market == config.market
            && entry.date == date
        {
            entry.path = path;
            return Some(entry);
        }
        // Legacy files don't carry region metadata; use them only for the legacy region.
        if config.market == "zh-CN"
            && path.file_name()?.to_str()? == format!("bing_wallpaper_{date}.jpg")
        {
            return Some(Wallpaper {
                path,
                title: "Bing 每日壁纸（原版缓存）".into(),
                copyright: String::new(),
                date: date.into(),
                market: config.market.clone(),
            });
        }
    }
    None
}

pub fn download(
    config: &Config,
    force: bool,
    progress: &impl Fn(String),
) -> Result<(Wallpaper, Vec<String>)> {
    config.validate()?;
    fs::create_dir_all(&config.directory).context("无法创建壁纸保存目录")?;
    let today = Local::now().format("%Y%m%d").to_string();
    if !force && let Some(wallpaper) = cached(config, &today) {
        progress("正在使用已校验的今日缓存…".into());
        return Ok((wallpaper, vec![]));
    }
    let client = Client::builder()
        .user_agent(concat!("BingWallpaperRust/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(30))
        .build()?;
    progress("正在获取 Bing 今日壁纸信息…".into());
    let mut errors = Vec::new();
    let mut item = None;
    for host in ["www.bing.com", "cn.bing.com"] {
        let url = format!(
            "https://{host}/HPImageArchive.aspx?format=js&idx=0&n=1&mkt={}",
            config.market
        );
        let result = (|| -> Result<ApiImage> {
            let bytes = fetch_bytes(&client, &url, 1024 * 1024)?;
            let response: ApiResponse =
                serde_json::from_slice(&bytes).context("Bing API 返回了非预期数据")?;
            let image = response
                .images
                .into_iter()
                .next()
                .context("Bing 没有返回壁纸")?;
            ensure!(valid_date(&image.startdate), "Bing 返回了无效日期");
            image_url(&image.urlbase, "UHD")?;
            Ok(image)
        })();
        match result {
            Ok(image) => {
                item = Some(image);
                break;
            }
            Err(e) => errors.push(format!("{host}: {e:#}")),
        }
    }
    let Some(item) = item else {
        if let Some(wallpaper) = cached(config, &today) {
            return Ok((
                wallpaper,
                vec![format!("网络不可用，使用今日缓存：{}", errors.join("；"))],
            ));
        }
        bail!("无法获取今日壁纸：\n{}", errors.join("\n"));
    };
    if !force && let Some(wallpaper) = cached(config, &item.startdate) {
        return Ok((wallpaper, errors));
    }
    let mut warnings = errors;
    let mut data = None;
    for resolution in ["UHD", "1920x1080"] {
        progress(format!("正在下载 {resolution} 壁纸并校验图片…"));
        let result = (|| -> Result<Vec<u8>> {
            let bytes = fetch_bytes(
                &client,
                image_url(&item.urlbase, resolution)?.as_str(),
                MAX_IMAGE_BYTES,
            )?;
            decode_image(&bytes)?;
            Ok(bytes)
        })();
        match result {
            Ok(bytes) => {
                data = Some(bytes);
                break;
            }
            Err(e) => warnings.push(format!("{resolution} 下载失败：{e:#}")),
        }
    }
    let bytes = data.with_context(|| format!("无法下载有效图片：{}", warnings.join("；")))?;
    let path = config.directory.join(format!(
        "bing_wallpaper_{}_{}.jpg",
        item.startdate, config.market
    ));
    let wallpaper = Wallpaper {
        path,
        title: item.title,
        copyright: item.copyright,
        date: item.startdate,
        market: config.market.clone(),
    };
    util::atomic_write(&wallpaper.path, &bytes)?;
    if let Err(e) = util::atomic_write(
        &wallpaper.path.with_extension("json"),
        &serde_json::to_vec_pretty(&wallpaper)?,
    ) {
        warnings.push(format!("图片已保存，元数据未保存：{e:#}"));
    }
    Ok((wallpaper, warnings))
}

/// Only our exact filenames are eligible. Never delete symlinks or unrelated images.
pub fn managed_date(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let body = name.strip_prefix("bing_wallpaper_")?.strip_suffix(".jpg")?;
    let (date, market) = body
        .split_once('_')
        .map_or((body, None), |(d, m)| (d, Some(m)));
    if !valid_date(date) {
        return None;
    }
    if let Some(m) = market {
        let b = m.as_bytes();
        if b.len() != 5
            || b[2] != b'-'
            || !b[..2].iter().chain(&b[3..]).all(u8::is_ascii_alphabetic)
        {
            return None;
        }
    }
    Some(date.to_owned())
}

pub fn cleanup(directory: &Path, keep: usize, current: &Path) -> Result<()> {
    ensure!(keep > 0, "保留数量不能为零");
    let mut files = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && let Some(date) = managed_date(&entry.path())
        {
            files.push((date, entry.path()));
        }
    }
    // Protect the applied file even if a clock correction made its date older.
    files.sort_by(|a, b| (b.1 == current, &b.0, &b.1).cmp(&(a.1 == current, &a.0, &a.1)));
    for (_, path) in files.into_iter().skip(keep) {
        fs::remove_file(&path)?;
        let metadata = path.with_extension("json");
        if metadata.is_file() && !metadata.is_symlink() {
            fs::remove_file(metadata)?;
        }
    }
    Ok(())
}

pub fn update(
    config: &Config,
    force: bool,
    download_only: bool,
    progress: impl Fn(String),
) -> Result<Update> {
    let _lock = util::lock(&config::state_root()?.join("update.lock"))?;
    let (wallpaper, mut warnings) = download(config, force, &progress)?;
    if download_only {
        return Ok(Update {
            wallpaper,
            method: "仅下载".into(),
            warnings,
            applied: false,
        });
    }
    progress("正在应用桌面壁纸…".into());
    let applied = backend::apply(&wallpaper.path, config.backend)
        .with_context(|| format!("图片已保存到 {}，但设置失败", wallpaper.path.display()))?;
    warnings.extend(applied.warnings);
    let state = State {
        wallpaper: Some(wallpaper.clone()),
        last_success: Some(Utc::now().timestamp()),
        method: applied.method.clone(),
    };
    if let Err(e) = state.save() {
        warnings.push(format!("壁纸已应用，但更新状态未保存：{e:#}"));
    }
    if let Err(e) = cleanup(&config.directory, config.keep_count, &wallpaper.path) {
        warnings.push(format!("旧图片清理未完成：{e:#}"));
    }
    Ok(Update {
        wallpaper,
        method: applied.method,
        warnings,
        applied: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_html_corruption_and_path_traversal() {
        assert!(decode_image(b"<html>gateway error</html>").is_err());
        assert!(!valid_date("20260230"));
        assert!(!valid_date("../../x"));
        assert!(image_url("//evil.test/a", "UHD").is_err());
        assert!(
            image_url("/th?id=OHR.Example_ZH-CN123", "UHD")
                .unwrap()
                .as_str()
                .ends_with("_UHD.jpg")
        );
    }
    #[test]
    fn cleanup_keeps_applied_image_and_user_files() {
        let dir = tempfile::tempdir().unwrap();
        let current = dir.path().join("bing_wallpaper_20260101.jpg");
        for name in [
            "bing_wallpaper_20260101.jpg",
            "bing_wallpaper_20260102_zh-CN.jpg",
            "bing_wallpaper_20260103_zh-CN.jpg",
            "family.jpg",
            "bing_wallpaper_notes.jpg",
            "bing_wallpaper_20260230.jpg",
        ] {
            fs::write(dir.path().join(name), "x").unwrap();
        }
        std::os::unix::fs::symlink(&current, dir.path().join("bing_wallpaper_20200101.jpg"))
            .unwrap();
        cleanup(dir.path(), 2, &current).unwrap();
        assert!(current.exists());
        assert!(
            !dir.path()
                .join("bing_wallpaper_20260102_zh-CN.jpg")
                .exists()
        );
        assert!(
            dir.path()
                .join("bing_wallpaper_20260103_zh-CN.jpg")
                .exists()
        );
        assert!(dir.path().join("family.jpg").exists());
        assert!(dir.path().join("bing_wallpaper_notes.jpg").exists());
        assert!(dir.path().join("bing_wallpaper_20260230.jpg").exists());
        assert!(dir.path().join("bing_wallpaper_20200101.jpg").is_symlink());
    }
    #[test]
    fn retry_and_size_limit_use_real_http_responses() {
        use std::{io::Write, net::TcpListener};
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            for status in ["503 Service Unavailable", "200 OK"] {
                let (mut conn, _) = listener.accept().unwrap();
                let mut buffer = [0; 2048];
                let _ = conn.read(&mut buffer);
                write!(
                    conn,
                    "HTTP/1.1 {status}\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello"
                )
                .unwrap();
            }
        });
        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        assert_eq!(fetch_bytes(&client, &url, 10).unwrap(), b"hello");
        server.join().unwrap();
    }
}
