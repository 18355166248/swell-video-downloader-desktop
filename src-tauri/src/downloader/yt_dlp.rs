use serde::Deserialize;
use std::{path::Path, process::Command};
use tauri::AppHandle;

use crate::platform::binaries::resolve_yt_dlp;

#[derive(Deserialize)]
pub struct YtDlpFormat {
    pub format_id: Option<String>,
    pub ext: Option<String>,
    pub height: Option<u64>,
    pub acodec: Option<String>,
    pub format_note: Option<String>,
}

#[derive(Deserialize)]
pub struct YtDlpMetadata {
    pub title: Option<String>,
    pub duration: Option<f64>,
    pub formats: Option<Vec<YtDlpFormat>>,
}

pub fn fetch_metadata(
    app: &AppHandle,
    url: &str,
    cookie_source: Option<&str>,
    cookie_file_path: Option<&str>,
) -> Result<YtDlpMetadata, String> {
    let binary = resolve_yt_dlp(app).ok_or_else(|| {
        "未找到 yt-dlp。请将其放到 resources/bin 目录，或通过 SWELL_YTDLP_PATH 指定路径。".to_string()
    })?;

    let mut command = Command::new(&binary.path);
    command.arg("-J");
    apply_cookie_source(&mut command, cookie_source, cookie_file_path)?;
    command.arg(url);

    let output = command
        .output()
        .map_err(|err| format!("无法启动 yt-dlp（{}）：{err}", binary.source))?;

    if !output.status.success() {
        return Err(normalize_yt_dlp_error(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }

    serde_json::from_slice(&output.stdout).map_err(|err| format!("解析 yt-dlp 输出失败：{err}"))
}

pub fn apply_cookie_source(
    command: &mut Command,
    cookie_source: Option<&str>,
    cookie_file_path: Option<&str>,
) -> Result<(), String> {
    match cookie_source.unwrap_or("chrome") {
        "chrome" => {
            command.arg("--cookies-from-browser");
            command.arg("chrome");
            Ok(())
        }
        "edge" => {
            command.arg("--cookies-from-browser");
            command.arg("edge");
            Ok(())
        }
        "import" => {
            let path = cookie_file_path
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "已选择手动导入 Cookie，请填写 cookies.txt 文件路径。".to_string())?;

            if !Path::new(path).is_file() {
                return Err("cookies.txt 文件不存在，请确认路径后重试。".into());
            }

            command.arg("--cookies");
            command.arg(path);
            Ok(())
        }
        "none" | "" => Ok(()),
        other => Err(format!("不支持的 Cookie 来源：{other}")),
    }
}

pub fn normalize_yt_dlp_error(raw: String) -> String {
    if raw.contains("Could not copy Chrome cookie database") {
        return "无法读取 Chrome Cookie：Chrome 当前正在占用 Cookies 数据库。请先完全关闭 Chrome 后重试，或后续改用手动导入 Cookie。".into();
    }

    if raw.contains("Could not copy Edge cookie database") {
        return "无法读取 Edge Cookie：Edge 当前正在占用 Cookies 数据库。请先完全关闭 Edge 后重试，或后续改用手动导入 Cookie。".into();
    }

    raw
}

#[cfg(test)]
mod tests {
    use super::apply_cookie_source;
    use std::{env, fs, process::Command};

    #[test]
    fn import_cookie_source_requires_file_path() {
        let mut command = Command::new("yt-dlp");
        let error = apply_cookie_source(&mut command, Some("import"), None)
            .expect_err("missing cookie file path should fail");

        assert!(error.contains("cookies.txt"));
    }

    #[test]
    fn import_cookie_source_rejects_missing_file() {
        let mut command = Command::new("yt-dlp");
        let error = apply_cookie_source(
            &mut command,
            Some("import"),
            Some("Z:\\definitely-missing\\cookies.txt"),
        )
        .expect_err("missing cookie file should fail");

        assert!(error.contains("不存在"));
    }

    #[test]
    fn import_cookie_source_adds_cookies_flag() {
        let cookie_path = env::temp_dir().join("swell-test-cookies.txt");
        fs::write(&cookie_path, "# Netscape HTTP Cookie File\n").expect("fixture should be writable");

        let mut command = Command::new("yt-dlp");
        apply_cookie_source(
            &mut command,
            Some("import"),
            Some(cookie_path.to_string_lossy().as_ref()),
        )
        .expect("valid cookie file path should be accepted");

        let program_and_args = format!("{command:?}");
        assert!(program_and_args.contains("--cookies"));
        assert!(program_and_args.contains("swell-test-cookies.txt"));

        let _ = fs::remove_file(cookie_path);
    }
}
