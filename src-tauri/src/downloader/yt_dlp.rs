use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::Path,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};
use tauri::AppHandle;

use crate::platform::binaries::resolve_yt_dlp;

/// Upper bound on a single yt-dlp metadata probe. Without it a stuck yt-dlp (slow
/// network extraction, a wedged `--cookies-from-browser` read) would hang resolve
/// forever with no feedback in the UI. On timeout we error out so x.com can fall
/// back to ssstwitter instead of spinning indefinitely.
const YT_DLP_METADATA_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Deserialize)]
pub struct YtDlpFormat {
    pub format_id: Option<String>,
    pub ext: Option<String>,
    pub height: Option<u64>,
    pub acodec: Option<String>,
    pub format_note: Option<String>,
    pub filesize: Option<u64>,
    pub filesize_approx: Option<u64>,
}

#[derive(Deserialize)]
pub struct YtDlpMetadata {
    pub title: Option<String>,
    pub duration: Option<f64>,
    pub formats: Option<Vec<YtDlpFormat>>,
    pub thumbnail: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct DiagnosticCommandPreview {
    pub program: String,
    pub args: Vec<String>,
    pub display_command: String,
}

#[derive(Clone, Serialize)]
pub struct FormatSummary {
    pub formats_count: usize,
    pub best_format_id: Option<String>,
    pub best_height: Option<u64>,
    pub max_height: Option<u64>,
    pub best_has_audio: bool,
    pub has_muxed_format: bool,
    pub has_video_only_format: bool,
    pub has_audio_only_format: bool,
}

#[derive(Clone, Serialize)]
pub struct YtDlpErrorInfo {
    pub error_category: String,
    pub normalized_message: String,
}

pub struct MetadataProbeSuccess {
    pub metadata: YtDlpMetadata,
    pub yt_dlp_source: String,
    pub proxy_enabled: bool,
    pub cookie_mode: String,
    pub command_preview: DiagnosticCommandPreview,
}

pub struct MetadataProbeFailure {
    pub error_info: YtDlpErrorInfo,
    pub raw_message: String,
    pub yt_dlp_source: String,
    pub proxy_enabled: bool,
    pub cookie_mode: String,
    pub command_preview: DiagnosticCommandPreview,
}

pub fn probe_metadata(
    app: &AppHandle,
    url: &str,
    cookie_source: Option<&str>,
    cookie_file_path: Option<&str>,
) -> Result<MetadataProbeSuccess, MetadataProbeFailure> {
    let binary = resolve_yt_dlp(app);
    let cookie_mode = current_cookie_mode(cookie_source);
    let detected_proxy = detected_proxy();
    let proxy_enabled = detected_proxy.is_some();
    let program = binary
        .as_ref()
        .map(|item| item.path.display().to_string())
        .unwrap_or_else(default_yt_dlp_program);
    let command_preview = build_metadata_probe(
        &program,
        url,
        Some(cookie_mode.as_str()),
        cookie_file_path,
        detected_proxy.clone(),
    )
    .unwrap_or_else(|_| fallback_metadata_probe(&program, url));
    let yt_dlp_source = binary
        .map(|item| item.source.to_string())
        .unwrap_or_else(|| "missing".into());

    let binary = resolve_yt_dlp(app).ok_or_else(|| MetadataProbeFailure {
        error_info: classify_yt_dlp_error(
            "未找到 yt-dlp。请将其放到 resources/bin 目录，或通过 SWELL_YTDLP_PATH 指定路径。",
        ),
        raw_message: "未找到 yt-dlp。请将其放到 resources/bin 目录，或通过 SWELL_YTDLP_PATH 指定路径。"
            .into(),
        yt_dlp_source: yt_dlp_source.clone(),
        proxy_enabled,
        cookie_mode: cookie_mode.clone(),
        command_preview: command_preview.clone(),
    })?;

    let mut command = Command::new(&binary.path);
    command.arg("-J");
    apply_proxy(&mut command);
    apply_cookie_source(&mut command, Some(cookie_mode.as_str()), cookie_file_path).map_err(
        |raw_message| MetadataProbeFailure {
            error_info: classify_yt_dlp_error(&raw_message),
            raw_message,
            yt_dlp_source: yt_dlp_source.clone(),
            proxy_enabled,
            cookie_mode: cookie_mode.clone(),
            command_preview: command_preview.clone(),
        },
    )?;
    command.arg(url);

    let output = output_with_timeout(command, YT_DLP_METADATA_TIMEOUT).map_err(|raw_message| {
        MetadataProbeFailure {
            error_info: classify_yt_dlp_error(&raw_message),
            raw_message,
            yt_dlp_source: yt_dlp_source.clone(),
            proxy_enabled,
            cookie_mode: cookie_mode.clone(),
            command_preview: command_preview.clone(),
        }
    })?;

    if !output.status.success() {
        let raw_message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(MetadataProbeFailure {
            error_info: classify_yt_dlp_error(&raw_message),
            raw_message,
            yt_dlp_source,
            proxy_enabled,
            cookie_mode,
            command_preview,
        });
    }

    let metadata = serde_json::from_slice(&output.stdout).map_err(|err| {
        let raw_message = format!("解析 yt-dlp 输出失败：{err}");
        MetadataProbeFailure {
            error_info: classify_yt_dlp_error(&raw_message),
            raw_message,
            yt_dlp_source: yt_dlp_source.clone(),
            proxy_enabled,
            cookie_mode: cookie_mode.clone(),
            command_preview: command_preview.clone(),
        }
    })?;

    Ok(MetadataProbeSuccess {
        metadata,
        yt_dlp_source,
        proxy_enabled,
        cookie_mode,
        command_preview,
    })
}

pub fn fetch_metadata(
    app: &AppHandle,
    url: &str,
    cookie_source: Option<&str>,
    cookie_file_path: Option<&str>,
) -> Result<YtDlpMetadata, String> {
    probe_metadata(app, url, cookie_source, cookie_file_path)
        .map(|success| success.metadata)
        .map_err(|failure| failure.error_info.normalized_message)
}

/// Run a command to completion but kill it if it exceeds `timeout`. stdout/stderr
/// are drained on dedicated threads so a child that fills a pipe buffer can't
/// deadlock while we poll for exit.
fn output_with_timeout(mut command: Command, timeout: Duration) -> Result<Output, String> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|err| format!("无法启动 yt-dlp：{err}"))?;

    let mut child_stdout = child.stdout.take().expect("piped stdout");
    let mut child_stderr = child.stderr.take().expect("piped stderr");
    let stdout_reader = thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = child_stdout.read_to_end(&mut buffer);
        buffer
    });
    let stderr_reader = thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = child_stderr.read_to_end(&mut buffer);
        buffer
    });

    let started_at = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if started_at.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "yt-dlp 解析超时（超过 {} 秒）。可能卡在读取浏览器 Cookie，可在设置里改用「无 Cookie」或手动导入 cookies.txt 后重试。",
                        timeout.as_secs()
                    ));
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(err) => return Err(format!("等待 yt-dlp 进程失败：{err}")),
        }
    };

    let stdout = stdout_reader.join().unwrap_or_default();
    let stderr = stderr_reader.join().unwrap_or_default();
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

/// Route yt-dlp through the detected system proxy (same rationale as the reqwest
/// client). Without it, yt-dlp connects directly to the site/CDN.
pub fn apply_proxy(command: &mut Command) {
    if let Some(proxy) = detected_proxy() {
        command.arg("--proxy");
        command.arg(proxy);
    }
}

pub fn current_cookie_mode(cookie_source: Option<&str>) -> String {
    match cookie_source.unwrap_or("chrome").trim() {
        "" => "none".into(),
        value => value.to_string(),
    }
}

pub fn apply_cookie_source(
    command: &mut Command,
    cookie_source: Option<&str>,
    cookie_file_path: Option<&str>,
) -> Result<(), String> {
    let args = cookie_args(cookie_source, cookie_file_path)?;
    command.args(args);
    Ok(())
}

pub fn normalize_yt_dlp_error(raw: String) -> String {
    classify_yt_dlp_error(&raw).normalized_message
}

pub fn classify_yt_dlp_error(raw: &str) -> YtDlpErrorInfo {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();

    if trimmed.contains("未找到 yt-dlp") {
        return YtDlpErrorInfo {
            error_category: "binary_missing".into(),
            normalized_message: trimmed.to_string(),
        };
    }

    if trimmed.contains("无法启动 yt-dlp") {
        return YtDlpErrorInfo {
            error_category: "spawn_failed".into(),
            normalized_message: "无法启动 yt-dlp，请检查可执行文件路径和权限。".into(),
        };
    }

    if trimmed.contains("yt-dlp 解析超时") {
        return YtDlpErrorInfo {
            error_category: "timeout".into(),
            normalized_message: trimmed.to_string(),
        };
    }

    if trimmed.contains("Could not copy Chrome cookie database") {
        return YtDlpErrorInfo {
            error_category: "cookie_locked".into(),
            normalized_message: "无法读取 Chrome Cookie：Chrome 当前正在占用 Cookies 数据库。请先完全关闭 Chrome 后重试，或后续改用手动导入 Cookie。".into(),
        };
    }

    if trimmed.contains("Could not copy Edge cookie database") {
        return YtDlpErrorInfo {
            error_category: "cookie_locked".into(),
            normalized_message: "无法读取 Edge Cookie：Edge 当前正在占用 Cookies 数据库。请先完全关闭 Edge 后重试，或后续改用手动导入 Cookie。".into(),
        };
    }

    if trimmed.contains("cookies.txt 文件不存在") || trimmed.contains("已选择手动导入 Cookie") {
        return YtDlpErrorInfo {
            error_category: "cookie_file_missing".into(),
            normalized_message: trimmed.to_string(),
        };
    }

    if lower.contains("geo restricted")
        || lower.contains("not available from your location")
        || trimmed.contains("地区")
    {
        return YtDlpErrorInfo {
            error_category: "geo_restricted".into(),
            normalized_message: "当前内容可能受地区限制，需切换可访问的网络环境后再试。".into(),
        };
    }

    if lower.contains("login")
        || lower.contains("sign in")
        || lower.contains("members only")
        || lower.contains("private video")
        || lower.contains("age verification")
        || trimmed.contains("年龄")
        || trimmed.contains("登录")
    {
        return YtDlpErrorInfo {
            error_category: "login_or_access_required".into(),
            normalized_message: "当前内容可能需要登录态、访问权限或年龄校验后才能解析。".into(),
        };
    }

    if lower.contains("proxy")
        || lower.contains("connection")
        || lower.contains("timed out")
        || lower.contains("network is unreachable")
        || lower.contains("name or service not known")
        || lower.contains("connection refused")
        || lower.contains("ssl")
    {
        return YtDlpErrorInfo {
            error_category: "proxy_or_network".into(),
            normalized_message: "连接目标站点或媒体资源失败，请检查代理、网络环境或证书配置。".into(),
        };
    }

    if lower.contains("unsupported url")
        || lower.contains("unable to extract")
        || lower.contains("extractor")
        || lower.contains("no video formats found")
        || trimmed.contains("解析 yt-dlp 输出失败")
    {
        return YtDlpErrorInfo {
            error_category: "extractor_changed".into(),
            normalized_message: "yt-dlp 当前未能正确提取该页面，可能需要更新 extractor 或排查页面结构变化。".into(),
        };
    }

    YtDlpErrorInfo {
        error_category: "unknown".into(),
        normalized_message: if trimmed.is_empty() {
            "未知错误，请查看日志或终端复现命令继续排查。".into()
        } else {
            trimmed.to_string()
        },
    }
}

pub fn build_metadata_probe(
    program: &str,
    url: &str,
    cookie_source: Option<&str>,
    cookie_file_path: Option<&str>,
    proxy: Option<String>,
) -> Result<DiagnosticCommandPreview, String> {
    let mut args = vec!["-J".to_string()];

    if let Some(proxy) = proxy.filter(|value| !value.trim().is_empty()) {
        args.push("--proxy".into());
        args.push(proxy);
    }

    args.extend(cookie_args(cookie_source, cookie_file_path)?);
    args.push(url.to_string());

    Ok(DiagnosticCommandPreview {
        program: program.to_string(),
        display_command: render_powershell_command(program, &args),
        args,
    })
}

pub fn summarize_formats(best_format_id: &str, formats: &[YtDlpFormat]) -> FormatSummary {
    let mut summary = FormatSummary {
        formats_count: formats.len(),
        best_format_id: if best_format_id.trim().is_empty() {
            None
        } else {
            Some(best_format_id.to_string())
        },
        best_height: None,
        max_height: None,
        best_has_audio: false,
        has_muxed_format: false,
        has_video_only_format: false,
        has_audio_only_format: false,
    };

    for format in formats {
        let has_video = format.height.is_some();
        let has_audio = format.acodec.as_deref().unwrap_or("none") != "none";

        if has_video && has_audio {
            summary.has_muxed_format = true;
        } else if has_video {
            summary.has_video_only_format = true;
        } else if has_audio {
            summary.has_audio_only_format = true;
        }

        if let Some(height) = format.height {
            summary.max_height = Some(summary.max_height.map_or(height, |current| current.max(height)));
        }

        if format.format_id.as_deref() == Some(best_format_id) {
            summary.best_height = format.height;
            summary.best_has_audio = has_audio;
        }
    }

    summary
}

fn cookie_args(
    cookie_source: Option<&str>,
    cookie_file_path: Option<&str>,
) -> Result<Vec<String>, String> {
    match current_cookie_mode(cookie_source).as_str() {
        "chrome" => Ok(vec!["--cookies-from-browser".into(), "chrome".into()]),
        "edge" => Ok(vec!["--cookies-from-browser".into(), "edge".into()]),
        "import" => {
            let path = cookie_file_path
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "已选择手动导入 Cookie，请填写 cookies.txt 文件路径。".to_string())?;

            if !Path::new(path).is_file() {
                return Err("cookies.txt 文件不存在，请确认路径后重试。".into());
            }

            Ok(vec!["--cookies".into(), path.to_string()])
        }
        "none" => Ok(Vec::new()),
        other => Err(format!("不支持的 Cookie 来源：{other}")),
    }
}

fn detected_proxy() -> Option<String> {
    crate::platform::proxy::detect_proxy()
}

fn default_yt_dlp_program() -> String {
    if cfg!(windows) {
        "yt-dlp.exe".into()
    } else {
        "yt-dlp".into()
    }
}

fn fallback_metadata_probe(program: &str, url: &str) -> DiagnosticCommandPreview {
    DiagnosticCommandPreview {
        program: program.to_string(),
        args: vec!["-J".into(), url.to_string()],
        display_command: render_powershell_command(program, &["-J".into(), url.to_string()]),
    }
}

fn render_powershell_command(program: &str, args: &[String]) -> String {
    std::iter::once(program)
        .chain(args.iter().map(String::as_str))
        .map(powershell_quote)
        .collect::<Vec<_>>()
        .join(" ")
}

fn powershell_quote(value: &str) -> String {
    let requires_quotes = value.chars().any(|ch| {
        ch.is_whitespace()
            || matches!(ch, '"' | '\'' | '&' | '(' | ')' | '[' | ']' | '{' | '}' | ';' | ',')
    });

    if requires_quotes {
        format!("'{}'", value.replace('\'', "''"))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        apply_cookie_source, build_metadata_probe, classify_yt_dlp_error, summarize_formats,
        YtDlpFormat,
    };
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

    #[test]
    fn metadata_probe_preview_keeps_cookie_free_command_minimal() {
        let preview = build_metadata_probe(
            "yt-dlp",
            "https://example.com/watch?v=123",
            Some("none"),
            None,
            None,
        )
        .expect("cookie-free probe should build");

        assert_eq!(preview.program, "yt-dlp");
        assert_eq!(preview.args, vec!["-J", "https://example.com/watch?v=123"]);
        assert_eq!(preview.display_command, "yt-dlp -J https://example.com/watch?v=123");
    }

    #[test]
    fn metadata_probe_preview_includes_browser_cookie_mode_and_proxy() {
        let preview = build_metadata_probe(
            "yt-dlp.exe",
            "https://example.com/watch?v=123",
            Some("chrome"),
            None,
            Some("http://127.0.0.1:7890".into()),
        )
        .expect("browser-cookie probe should build");

        assert_eq!(
            preview.args,
            vec![
                "-J",
                "--proxy",
                "http://127.0.0.1:7890",
                "--cookies-from-browser",
                "chrome",
                "https://example.com/watch?v=123",
            ]
        );
        assert!(preview.display_command.contains("--cookies-from-browser chrome"));
        assert!(preview.display_command.contains("--proxy http://127.0.0.1:7890"));
    }

    #[test]
    fn classify_timeout_as_timeout_category() {
        let info = classify_yt_dlp_error("yt-dlp 解析超时（超过 30 秒）");

        assert_eq!(info.error_category, "timeout");
        assert!(info.normalized_message.contains("超时"));
    }

    #[test]
    fn summarize_formats_reports_best_and_max_height() {
        let summary = summarize_formats(
            "137",
            &[
                YtDlpFormat {
                    format_id: Some("137".into()),
                    ext: Some("mp4".into()),
                    height: Some(1080),
                    acodec: Some("none".into()),
                    format_note: None,
                    filesize: None,
                    filesize_approx: None,
                },
                YtDlpFormat {
                    format_id: Some("22".into()),
                    ext: Some("mp4".into()),
                    height: Some(720),
                    acodec: Some("aac".into()),
                    format_note: None,
                    filesize: None,
                    filesize_approx: None,
                },
                YtDlpFormat {
                    format_id: Some("140".into()),
                    ext: Some("m4a".into()),
                    height: None,
                    acodec: Some("aac".into()),
                    format_note: None,
                    filesize: None,
                    filesize_approx: None,
                },
            ],
        );

        assert_eq!(summary.formats_count, 3);
        assert_eq!(summary.best_format_id.as_deref(), Some("137"));
        assert_eq!(summary.best_height, Some(1080));
        assert_eq!(summary.max_height, Some(1080));
        assert!(!summary.best_has_audio);
        assert!(summary.has_muxed_format);
        assert!(summary.has_video_only_format);
        assert!(summary.has_audio_only_format);
    }
}
