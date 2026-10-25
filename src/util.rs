use anyhow::{Context, Result, bail};
use fs2::FileExt;
use std::{
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path.parent().context("文件路径没有父目录")?;
    fs::create_dir_all(parent).with_context(|| format!("无法创建 {}", parent.display()))?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(data)?;
    temp.as_file().sync_all()?;
    temp.persist(path)
        .with_context(|| format!("无法保存 {}", path.display()))?;
    Ok(())
}

pub fn lock(path: &Path) -> Result<File> {
    fs::create_dir_all(path.parent().context("无效的锁文件路径")?)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.try_lock_exclusive()
        .context("已有任务正在运行，请稍后再试")?;
    Ok(file)
}

pub fn executable(name: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let is_executable = |p: &Path| {
        p.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    };
    if name.contains('/') {
        return is_executable(Path::new(name)).then(|| PathBuf::from(name));
    }
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|p| p.join(name))
        .find(|p| is_executable(p))
}

/// No shell interpolation. Drain both pipes while polling so a verbose child cannot deadlock.
pub fn run(program: impl AsRef<OsStr>, args: &[&str], timeout: Duration) -> Result<String> {
    let program = program.as_ref();
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("无法启动 {}", program.to_string_lossy()))?;
    fn drain(mut pipe: impl Read + Send + 'static) -> std::sync::mpsc::Receiver<Vec<u8>> {
        let (tx, rx) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let mut out = Vec::new();
            let mut buf = [0; 4096];
            while let Ok(n) = pipe.read(&mut buf) {
                if n == 0 {
                    break;
                }
                if out.len() < 65536 {
                    out.extend_from_slice(&buf[..n.min(65536 - out.len())]);
                }
            }
            let _ = tx.send(out);
        });
        rx
    }
    let stdout = drain(child.stdout.take().context("无法读取命令输出")?);
    let stderr = drain(child.stderr.take().context("无法读取命令错误")?);
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            bail!(
                "{} 执行超时（{} 秒）",
                program.to_string_lossy(),
                timeout.as_secs()
            );
        }
        thread::sleep(Duration::from_millis(20));
    };
    // Descendants may inherit stdout/stderr after the original child exits.
    // Never wait indefinitely for such pipes to close.
    let out = stdout
        .recv_timeout(timeout.saturating_sub(start.elapsed()))
        .context("命令输出管道未在超时前关闭")?;
    let err = stderr
        .recv_timeout(timeout.saturating_sub(start.elapsed()))
        .context("命令错误管道未在超时前关闭")?;
    let out = String::from_utf8_lossy(&out).trim().to_owned();
    let err = String::from_utf8_lossy(&err).trim().to_owned();
    if !status.success() {
        bail!(
            "{} 退出状态 {}：{} {}",
            program.to_string_lossy(),
            status,
            err,
            out
        );
    }
    Ok(out)
}

pub fn file_uri(path: &Path) -> Result<String> {
    let absolute = path
        .canonicalize()
        .with_context(|| format!("无法读取 {}", path.display()))?;
    url::Url::from_file_path(absolute)
        .map(String::from)
        .map_err(|_| anyhow::anyhow!("无法转换文件 URI"))
}

pub fn open_path(path: &Path) -> Result<()> {
    let path = path.to_str().context("路径不是有效 UTF-8")?;
    run("xdg-open", &[path], Duration::from_secs(15))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atomic_replacement_and_exclusive_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings");
        atomic_write(&path, b"first").unwrap();
        atomic_write(&path, b"second").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
        let held = lock(&dir.path().join("lock")).unwrap();
        assert!(lock(&dir.path().join("lock")).is_err());
        drop(held);
        assert!(lock(&dir.path().join("lock")).is_ok());
    }
    #[test]
    fn commands_are_bounded_and_report_stderr() {
        let start = Instant::now();
        assert!(
            run("sleep", &["2"], Duration::from_millis(40))
                .unwrap_err()
                .to_string()
                .contains("超时")
        );
        assert!(start.elapsed() < Duration::from_secs(1));
        assert!(
            run(
                "ls",
                &["/nonexistent-bing-wallpaper-test"],
                Duration::from_secs(2)
            )
            .is_err()
        );
    }
    #[test]
    fn file_uri_encodes_special_characters() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("中文 '#?.jpg");
        fs::write(&path, b"x").unwrap();
        let uri = file_uri(&path).unwrap();
        assert!(uri.contains("%20") && uri.contains("%23") && uri.contains("%3F"));
        assert_eq!(url::Url::parse(&uri).unwrap().to_file_path().unwrap(), path);
    }
}
