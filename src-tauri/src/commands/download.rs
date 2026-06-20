use serde::Serialize;
use std::{
    fs,
    io::{BufRead, BufReader, Read},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    downloader::x_ssstwitter::{
        download_x_via_ssstwitter_to_path, extract_ssstwitter_selection_label,
    },
    events::download_events::{DOWNLOAD_ERROR, DOWNLOAD_PROGRESS, DOWNLOAD_STATUS},
    platform::binaries::{resolve_ffmpeg, resolve_yt_dlp},
    downloader::yt_dlp::{apply_cookie_source, normalize_yt_dlp_error},
};

static DOWNLOAD_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Serialize)]
struct DownloadStatusPayload {
    task_id: String,
    title: String,
    status: String,
    message: Option<String>,
    output_path: Option<String>,
}

#[derive(Clone, Serialize)]
struct DownloadProgressPayload {
    task_id: String,
    percent: String,
    speed: String,
    eta: String,
}

#[tauri::command]
pub fn start_download(
    app: AppHandle,
    url: String,
    format_id: Option<String>,
    title: Option<String>,
    cookie_source: Option<String>,
    cookie_file_path: Option<String>,
) -> Result<String, String> {
    if url.trim().is_empty() {
        return Err("下载地址不能为空".into());
    }

    let task_id = format!("task-{}", DOWNLOAD_COUNTER.fetch_add(1, Ordering::Relaxed));
    let task_title = title.unwrap_or_else(|| "新下载任务".into());

    emit_status(
        &app,
        DownloadStatusPayload {
            task_id: task_id.clone(),
            title: task_title.clone(),
            status: "queued".into(),
            message: None,
            output_path: None,
        },
    );

    let download_dir = resolve_download_dir(&app)?;
    let yt_dlp = resolve_yt_dlp(&app).ok_or_else(|| {
        "未找到 yt-dlp。请将其放到 resources/bin 目录，或通过 SWELL_YTDLP_PATH 指定路径。"
            .to_string()
    })?;
    let ffmpeg = resolve_ffmpeg(&app).ok_or_else(|| {
        "未找到 ffmpeg。请将其放到 resources/bin 目录，或通过 SWELL_FFMPEG_PATH 指定路径。"
            .to_string()
    })?;

    let app_handle = app.clone();
    let return_task_id = task_id.clone();
    thread::spawn(move || {
        if let Err(message) = run_download_task(
            app_handle.clone(),
            task_id.clone(),
            task_title.clone(),
            url,
            format_id,
            cookie_source,
            cookie_file_path,
            yt_dlp.path,
            ffmpeg.path,
            download_dir,
        ) {
            emit_error(
                &app_handle,
                DownloadStatusPayload {
                    task_id,
                    title: task_title,
                    status: "failed".into(),
                    message: Some(message),
                    output_path: None,
                },
            );
        }
    });

    Ok(return_task_id)
}

fn run_download_task(
    app: AppHandle,
    task_id: String,
    title: String,
    url: String,
    format_id: Option<String>,
    cookie_source: Option<String>,
    cookie_file_path: Option<String>,
    yt_dlp_path: PathBuf,
    ffmpeg_path: PathBuf,
    download_dir: PathBuf,
) -> Result<(), String> {
    fs::create_dir_all(&download_dir).map_err(|err| format!("创建下载目录失败：{err}"))?;

    if url.contains("x.com") {
        if let Some(selection_label) = format_id
            .as_deref()
            .and_then(extract_ssstwitter_selection_label)
        {
            return run_ssstwitter_download_task(
                app,
                task_id,
                title,
                url,
                &selection_label,
                download_dir,
            );
        }
    }

    let mut command = Command::new(&yt_dlp_path);
    command.arg("--newline");
    command.arg("--progress-template");
    command.arg("download:__PROGRESS__:%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s");
    command.arg("--print");
    command.arg("after_move:__FINAL_PATH__:%(filepath)s");
    command.arg("-o");
    command.arg(download_dir.join("%(title)s [%(id)s].%(ext)s"));

    let direct_download_url = direct_download_url(format_id.as_deref());

    if direct_download_url.is_none() {
        if let Some(format) = format_id.as_ref().filter(|value| !value.trim().is_empty()) {
            command.arg("-f");
            command.arg(format);
        }
    }

    apply_cookie_source(
        &mut command,
        cookie_source.as_deref(),
        cookie_file_path.as_deref(),
    )?;

    if let Some(parent_dir) = ffmpeg_path.parent() {
        command.arg("--ffmpeg-location");
        command.arg(parent_dir);
    }

    command.arg(direct_download_url.unwrap_or(url));
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|err| format!("启动下载任务失败：{err}"))?;

    emit_status(
        &app,
        DownloadStatusPayload {
            task_id: task_id.clone(),
            title: title.clone(),
            status: "downloading".into(),
            message: None,
            output_path: None,
        },
    );

    let final_path = Arc::new(Mutex::new(None::<String>));
    let last_error = Arc::new(Mutex::new(None::<String>));

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let stdout_handle = stdout.map(|reader| {
        let app = app.clone();
        let task_id = task_id.clone();
        let title = title.clone();
        let final_path = final_path.clone();
        let last_error = last_error.clone();
        thread::spawn(move || {
            pump_reader(reader, &app, &task_id, &title, &final_path, &last_error);
        })
    });

    let stderr_handle = stderr.map(|reader| {
        let app = app.clone();
        let task_id = task_id.clone();
        let title = title.clone();
        let final_path = final_path.clone();
        let last_error = last_error.clone();
        thread::spawn(move || {
            pump_reader(reader, &app, &task_id, &title, &final_path, &last_error);
        })
    });

    let status = child
        .wait()
        .map_err(|err| format!("等待下载进程结束失败：{err}"))?;

    if let Some(handle) = stdout_handle {
        let _ = handle.join();
    }

    if let Some(handle) = stderr_handle {
        let _ = handle.join();
    }

    let output_path = final_path
        .lock()
        .ok()
        .and_then(|value| value.clone());

    if status.success() {
        emit_status(
            &app,
            DownloadStatusPayload {
                task_id,
                title,
                status: "completed".into(),
                message: Some("下载完成".into()),
                output_path,
            },
        );
        Ok(())
    } else {
        let message = last_error
            .lock()
            .ok()
            .and_then(|value| value.clone())
            .map(normalize_yt_dlp_error)
            .unwrap_or_else(|| "下载失败，请查看日志输出".into());

        emit_error(
            &app,
            DownloadStatusPayload {
                task_id,
                title,
                status: "failed".into(),
                message: Some(message.clone()),
                output_path,
            },
        );
        Err(message)
    }
}

fn pump_reader<R: Read>(
    reader: R,
    app: &AppHandle,
    task_id: &str,
    title: &str,
    final_path: &Arc<Mutex<Option<String>>>,
    last_error: &Arc<Mutex<Option<String>>>,
) {
    for line in BufReader::new(reader).lines().map_while(Result::ok) {
        handle_download_line(app, task_id, title, &line, final_path, last_error);
    }
}

fn handle_download_line(
    app: &AppHandle,
    task_id: &str,
    title: &str,
    line: &str,
    final_path: &Arc<Mutex<Option<String>>>,
    last_error: &Arc<Mutex<Option<String>>>,
) {
    if let Some(payload) = line.strip_prefix("__PROGRESS__:") {
        let mut parts = payload.split('|');
        let percent = parts.next().unwrap_or("0%").trim().to_string();
        let speed = parts.next().unwrap_or("--").trim().to_string();
        let eta = parts.next().unwrap_or("--").trim().to_string();

        let _ = app.emit(
            DOWNLOAD_PROGRESS,
            DownloadProgressPayload {
                task_id: task_id.to_string(),
                percent,
                speed,
                eta,
            },
        );
        return;
    }

    if let Some(path) = line.strip_prefix("__FINAL_PATH__:") {
        if let Ok(mut stored) = final_path.lock() {
            *stored = Some(path.trim().to_string());
        }
        return;
    }

    if line.contains("[Merger]") || line.contains("Merging formats into") || line.contains("[ExtractAudio]") {
        emit_status(
            app,
            DownloadStatusPayload {
                task_id: task_id.to_string(),
                title: title.to_string(),
                status: "postprocessing".into(),
                message: Some(line.trim().to_string()),
                output_path: None,
            },
        );
        return;
    }

    if line.contains("ERROR:") {
        if let Ok(mut stored) = last_error.lock() {
            *stored = Some(line.trim().to_string());
        }
    }
}

fn emit_status(app: &AppHandle, payload: DownloadStatusPayload) {
    let _ = app.emit(DOWNLOAD_STATUS, payload);
}

fn emit_error(app: &AppHandle, payload: DownloadStatusPayload) {
    let _ = app.emit(DOWNLOAD_ERROR, payload.clone());
    let _ = app.emit(DOWNLOAD_STATUS, payload);
}

fn resolve_download_dir(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(path) = app.path().download_dir() {
        return Ok(path);
    }

    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    Ok(project_root.join("downloads"))
}

fn run_ssstwitter_download_task(
    app: AppHandle,
    task_id: String,
    title: String,
    url: String,
    selection_label: &str,
    download_dir: PathBuf,
) -> Result<(), String> {
    emit_status(
        &app,
        DownloadStatusPayload {
            task_id: task_id.clone(),
            title: title.clone(),
            status: "downloading".into(),
            message: Some(format!("正在通过 ssstwitter 回退下载：{selection_label}")),
            output_path: None,
        },
    );

    let output_path = download_dir.join(format!(
        "{}.{}",
        sanitize_filename(&format!("{title} [{selection_label}]")),
        "mp4"
    ));
    let started_at = Instant::now();
    let mut last_emit_at = Instant::now() - Duration::from_secs(1);
    let result = download_x_via_ssstwitter_to_path(
        &url,
        Some(selection_label),
        &output_path,
        None,
        |downloaded_bytes, total_bytes| {
            if last_emit_at.elapsed() >= Duration::from_millis(300) {
                emit_progress(
                    &app,
                    task_id.as_str(),
                    downloaded_bytes,
                    total_bytes,
                    started_at,
                );
                last_emit_at = Instant::now();
            }
        },
    )?;

    emit_progress(
        &app,
        task_id.as_str(),
        result.downloaded_bytes,
        result.total_bytes,
        started_at,
    );

    emit_status(
        &app,
        DownloadStatusPayload {
            task_id,
            title,
            status: "completed".into(),
            message: Some("下载完成".into()),
            output_path: Some(output_path.display().to_string()),
        },
    );
    Ok(())
}

fn direct_download_url(format_id: Option<&str>) -> Option<String> {
    format_id
        .map(str::trim)
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        .map(str::to_string)
}

fn emit_progress(
    app: &AppHandle,
    task_id: &str,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    started_at: Instant,
) {
    let elapsed = started_at.elapsed().as_secs_f64().max(0.001);
    let bytes_per_second = downloaded_bytes as f64 / elapsed;
    let speed = human_speed(bytes_per_second);

    let (percent, eta) = if let Some(total) = total_bytes.filter(|value| *value > 0) {
        let percent = format!("{:.1}%", downloaded_bytes as f64 * 100.0 / total as f64);
        let remaining = total.saturating_sub(downloaded_bytes) as f64;
        let eta_seconds = if bytes_per_second > 0.0 {
            remaining / bytes_per_second
        } else {
            0.0
        };
        (percent, format_eta(eta_seconds))
    } else {
        ("--".into(), "--".into())
    };

    let _ = app.emit(
        DOWNLOAD_PROGRESS,
        DownloadProgressPayload {
            task_id: task_id.to_string(),
            percent,
            speed,
            eta,
        },
    );
}

fn human_speed(bytes_per_second: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;

    if bytes_per_second >= MB {
        format!("{:.1} MiB/s", bytes_per_second / MB)
    } else if bytes_per_second >= KB {
        format!("{:.1} KiB/s", bytes_per_second / KB)
    } else {
        format!("{:.0} B/s", bytes_per_second)
    }
}

fn format_eta(seconds: f64) -> String {
    let total_seconds = seconds.round().max(0.0) as u64;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes:02}:{seconds:02}")
}

fn sanitize_filename(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => character,
        })
        .collect::<String>()
        .trim()
        .trim_end_matches('.')
        .to_string();

    if sanitized.is_empty() {
        "download".into()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::{direct_download_url, extract_ssstwitter_selection_label, format_eta, sanitize_filename};

    #[test]
    fn detects_http_download_url_from_format_id() {
        let result = direct_download_url(Some("https://ssscdn.io/example/video.mp4?token=1"));

        assert_eq!(
            result.as_deref(),
            Some("https://ssscdn.io/example/video.mp4?token=1")
        );
    }

    #[test]
    fn ignores_regular_format_id_values() {
        assert_eq!(direct_download_url(Some("best")), None);
        assert_eq!(direct_download_url(Some("1080p")), None);
        assert_eq!(direct_download_url(Some("  mp4-high  ")), None);
    }

    #[test]
    fn ignores_ssstwitter_selection_ids_for_direct_url_detection() {
        assert_eq!(
            direct_download_url(Some("ssstwitter:下载 HD 1080x1080")),
            None
        );
        assert_eq!(
            extract_ssstwitter_selection_label("ssstwitter:下载 HD 1080x1080").as_deref(),
            Some("下载 HD 1080x1080")
        );
    }

    #[test]
    fn sanitizes_windows_unsafe_filename_characters() {
        assert_eq!(
            sanitize_filename("X 视频: 测试/1080p?"),
            "X 视频_ 测试_1080p_"
        );
    }

    #[test]
    fn formats_eta_as_minutes_and_seconds() {
        assert_eq!(format_eta(5.0), "00:05");
        assert_eq!(format_eta(65.0), "01:05");
    }
}
