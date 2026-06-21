use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Condvar, Mutex, OnceLock,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    downloader::x_ssstwitter::{
        download_selection_to_path, extract_ssstwitter_selection, SssTwitterSelection,
    },
    events::download_events::{DOWNLOAD_ERROR, DOWNLOAD_PROGRESS, DOWNLOAD_STATUS},
    platform::binaries::{resolve_ffmpeg, resolve_yt_dlp},
    downloader::yt_dlp::{apply_cookie_source, apply_proxy, normalize_yt_dlp_error},
};

static DOWNLOAD_COUNTER: AtomicU64 = AtomicU64::new(1);
static SSSTWITTER_DOWNLOAD_SLOT: OnceLock<Mutex<()>> = OnceLock::new();
static DOWNLOAD_TASKS: OnceLock<Mutex<HashMap<String, DownloadTaskControl>>> = OnceLock::new();
static DOWNLOAD_PERMITS: OnceLock<(Mutex<usize>, Condvar)> = OnceLock::new();

/// Cap on simultaneously running downloads. Batch actions (e.g. "download all 50")
/// enqueue many tasks at once; without a cap they would each spawn a yt-dlp
/// process immediately and swamp the network/CPU. Extra tasks wait as "queued".
const MAX_CONCURRENT_DOWNLOADS: usize = 3;

fn download_permits() -> &'static (Mutex<usize>, Condvar) {
    DOWNLOAD_PERMITS.get_or_init(|| (Mutex::new(MAX_CONCURRENT_DOWNLOADS), Condvar::new()))
}

/// Holds one download permit; releases it back to the pool on drop.
struct DownloadPermit;

impl Drop for DownloadPermit {
    fn drop(&mut self) {
        let (mutex, condvar) = download_permits();
        if let Ok(mut available) = mutex.lock() {
            *available += 1;
            condvar.notify_one();
        }
    }
}

/// Acquire a permit, waiting while the pool is empty. Returns `None` if the task
/// is canceled while still waiting in the queue.
fn acquire_download_permit(cancel_requested: &AtomicBool) -> Option<DownloadPermit> {
    let (mutex, condvar) = download_permits();
    let mut available = mutex.lock().ok()?;
    while *available == 0 {
        if cancel_requested.load(Ordering::SeqCst) {
            return None;
        }
        let (guard, _timeout) = condvar
            .wait_timeout(available, Duration::from_millis(200))
            .ok()?;
        available = guard;
    }
    *available -= 1;
    Some(DownloadPermit)
}

#[derive(Clone)]
struct DownloadTaskControl {
    title: String,
    cancel_requested: Arc<AtomicBool>,
}

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

#[derive(Default, Deserialize, Serialize, Clone)]
#[serde(default)]
struct AppSettings {
    download_dir: Option<String>,
    cookie_source: Option<String>,
    cookie_file_path: Option<String>,
    // sessionid is stored base64-encoded as a minimal visibility protection so it
    // is not sitting in the config file as plain readable text.
    instagram_sessionid_b64: Option<String>,
    instagram_cookie_file_path: Option<String>,
    instagram_collect_mode: Option<String>,
    instagram_collect_count: Option<String>,
    auto_download: Option<bool>,
}

/// Settings exposed to the UI (sessionid decoded back to plain text).
#[derive(Clone, Serialize)]
pub struct AppSettingsPayload {
    pub cookie_source: Option<String>,
    pub cookie_file_path: Option<String>,
    pub instagram_sessionid: Option<String>,
    pub instagram_cookie_file_path: Option<String>,
    pub instagram_collect_mode: Option<String>,
    pub instagram_collect_count: Option<String>,
    pub auto_download: Option<bool>,
}

/// Full snapshot of UI-editable settings sent on every save.
#[derive(Deserialize)]
pub struct AppSettingsUpdate {
    pub cookie_source: Option<String>,
    pub cookie_file_path: Option<String>,
    pub instagram_sessionid: Option<String>,
    pub instagram_cookie_file_path: Option<String>,
    pub instagram_collect_mode: Option<String>,
    pub instagram_collect_count: Option<String>,
    pub auto_download: Option<bool>,
}

#[derive(Clone, Serialize)]
pub struct DownloadDirectorySettings {
    pub current_dir: String,
    pub default_dir: String,
    pub is_custom: bool,
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
    eprintln!(
        "[start_download] url={url} format_id={:?} title={:?} cookie_source={:?} cookie_file_path={:?} requires_binary_toolchain={}",
        format_id,
        title,
        cookie_source,
        cookie_file_path,
        requires_binary_toolchain(&url, format_id.as_deref())
    );
    if url.trim().is_empty() {
        eprintln!("[start_download] rejected empty url");
        return Err("下载地址不能为空".into());
    }

    let task_id = format!("task-{}", DOWNLOAD_COUNTER.fetch_add(1, Ordering::Relaxed));
    let task_title = title.unwrap_or_else(|| "新下载任务".into());
    let cancel_requested = Arc::new(AtomicBool::new(false));

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
    eprintln!("[start_download] resolved download_dir={}", download_dir.display());

    let app_handle = app.clone();
    let return_task_id = task_id.clone();
    register_download_task(
        task_id.clone(),
        task_title.clone(),
        cancel_requested.clone(),
    )?;
    eprintln!("[start_download] registered task_id={task_id} title={task_title}");
    thread::spawn(move || {
        let result = run_download_task(
            app_handle.clone(),
            task_id.clone(),
            task_title.clone(),
            url,
            format_id,
            cookie_source,
            cookie_file_path,
            download_dir,
            cancel_requested,
        );
        unregister_download_task(&task_id);
        if let Err(message) = result {
            eprintln!("[start_download] task_id={task_id} failed: {message}");
            if message == "下载已取消" {
                emit_status(
                    &app_handle,
                    DownloadStatusPayload {
                        task_id,
                        title: task_title,
                        status: "canceled".into(),
                        message: Some(message),
                        output_path: None,
                    },
                );
            } else {
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
        }
    });

    Ok(return_task_id)
}

#[tauri::command]
pub fn cancel_download(app: AppHandle, task_id: String) -> Result<(), String> {
    let control = {
        let tasks = download_tasks()
            .lock()
            .map_err(|_| "下载任务状态异常。".to_string())?;
        tasks.get(&task_id).cloned()
    }
    .ok_or_else(|| "下载任务不存在或已结束。".to_string())?;

    control.cancel_requested.store(true, Ordering::SeqCst);
    emit_status(
        &app,
        DownloadStatusPayload {
            task_id,
            title: control.title,
            status: "canceling".into(),
            message: Some("正在取消下载…".into()),
            output_path: None,
        },
    );
    Ok(())
}

fn register_download_task(
    task_id: String,
    title: String,
    cancel_requested: Arc<AtomicBool>,
) -> Result<(), String> {
    let mut tasks = download_tasks()
        .lock()
        .map_err(|_| "下载任务状态异常。".to_string())?;
    tasks.insert(
        task_id,
        DownloadTaskControl {
            title,
            cancel_requested,
        },
    );
    Ok(())
}

fn unregister_download_task(task_id: &str) {
    if let Ok(mut tasks) = download_tasks().lock() {
        tasks.remove(task_id);
    }
}

fn download_tasks() -> &'static Mutex<HashMap<String, DownloadTaskControl>> {
    DOWNLOAD_TASKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn run_download_task(
    app: AppHandle,
    task_id: String,
    title: String,
    url: String,
    format_id: Option<String>,
    cookie_source: Option<String>,
    cookie_file_path: Option<String>,
    download_dir: PathBuf,
    cancel_requested: Arc<AtomicBool>,
) -> Result<(), String> {
    eprintln!(
        "[run_download_task] task_id={task_id} url={url} format_id={:?} title={title}",
        format_id
    );
    // Wait for a concurrency slot so batch downloads don't all start at once.
    // The task stays "queued" (already emitted) until a permit frees up.
    let _permit = match acquire_download_permit(&cancel_requested) {
        Some(permit) => permit,
        None => return Err("下载已取消".into()),
    };

    // Land downloads under <base>/<site>/, then sort each finished file into a
    // resource-type subfolder (视频 / 图片 / 音频 / 其他) by its extension.
    let site_dir = site_target_dir(&download_dir, &url);
    fs::create_dir_all(&site_dir).map_err(|err| format!("创建下载目录失败：{err}"))?;

    if url.contains("x.com") {
        if let Some(selection) = format_id.as_deref().and_then(extract_ssstwitter_selection) {
            eprintln!(
                "[run_download_task] task_id={task_id} using ssstwitter selection label={} direct_url_present={}",
                selection.label,
                selection.direct_url.is_some()
            );
            return run_ssstwitter_download_task(
                app,
                task_id,
                title,
                url,
                selection,
                site_dir,
                cancel_requested,
            );
        }
        eprintln!("[run_download_task] task_id={task_id} x.com url but no ssstwitter selection decoded");
    }

    let yt_dlp = resolve_yt_dlp(&app).ok_or_else(|| {
        eprintln!("[run_download_task] task_id={task_id} yt-dlp not found");
        "未找到 yt-dlp。请将其放到 resources/bin 目录，或通过 SWELL_YTDLP_PATH 指定路径。"
            .to_string()
    })?;
    eprintln!("[run_download_task] task_id={task_id} yt_dlp={}", yt_dlp.path.display());
    let ffmpeg = resolve_ffmpeg(&app).ok_or_else(|| {
        eprintln!("[run_download_task] task_id={task_id} ffmpeg not found");
        "未找到 ffmpeg。请将其放到 resources/bin 目录，或通过 SWELL_FFMPEG_PATH 指定路径。"
            .to_string()
    })?;
    eprintln!("[run_download_task] task_id={task_id} ffmpeg={}", ffmpeg.path.display());

    let mut command = Command::new(&yt_dlp.path);
    apply_download_progress_args(&mut command);
    command.arg("--print");
    command.arg("after_move:__FINAL_PATH__:%(filepath)s");
    command.arg("-o");
    command.arg(site_dir.join(format!("{}.%(ext)s", sanitize_filename(&title))));

    let direct_download_url = direct_download_url(format_id.as_deref());

    if direct_download_url.is_none() {
        if let Some(format) = format_id.as_ref().filter(|value| !value.trim().is_empty()) {
            command.arg("-f");
            command.arg(format);
        }
    }

    apply_proxy(&mut command);
    apply_cookie_source(
        &mut command,
        cookie_source.as_deref(),
        cookie_file_path.as_deref(),
    )?;

    if let Some(parent_dir) = ffmpeg.path.parent() {
        command.arg("--ffmpeg-location");
        command.arg(parent_dir);
    }

    command.arg(direct_download_url.unwrap_or(url));
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|err| {
            eprintln!("[run_download_task] task_id={task_id} failed to spawn yt-dlp: {err}");
            format!("启动下载任务失败：{err}")
        })?;

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

    let mut stdout_handle = stdout.map(|reader| {
        let app = app.clone();
        let task_id = task_id.clone();
        let title = title.clone();
        let final_path = final_path.clone();
        let last_error = last_error.clone();
        thread::spawn(move || {
            pump_reader(reader, &app, &task_id, &title, &final_path, &last_error);
        })
    });

    let mut stderr_handle = stderr.map(|reader| {
        let app = app.clone();
        let task_id = task_id.clone();
        let title = title.clone();
        let final_path = final_path.clone();
        let last_error = last_error.clone();
        thread::spawn(move || {
            pump_reader(reader, &app, &task_id, &title, &final_path, &last_error);
        })
    });

    let status = loop {
        if cancel_requested.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            if let Some(handle) = stdout_handle.take() {
                let _ = handle.join();
            }
            if let Some(handle) = stderr_handle.take() {
                let _ = handle.join();
            }
            return Err("下载已取消".into());
        }

        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(150)),
            Err(err) => return Err(format!("等待下载进程结束失败：{err}")),
        }
    };

    if let Some(handle) = stdout_handle.take() {
        let _ = handle.join();
    }

    if let Some(handle) = stderr_handle.take() {
        let _ = handle.join();
    }

    let output_path = final_path
        .lock()
        .ok()
        .and_then(|value| value.clone());

    if status.success() {
        // Sort the finished file into its resource-type subfolder by extension.
        let output_path = output_path.map(|path| relocate_into_category(&path, &site_dir));
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

/// The directory finished downloads land in. Created on access so the UI can open
/// it even before the first download completes.
#[tauri::command(async)]
pub fn get_download_dir(app: AppHandle) -> Result<String, String> {
    let dir = resolve_download_dir(&app)?;
    fs::create_dir_all(&dir).map_err(|err| format!("创建下载目录失败：{err}"))?;
    Ok(dir.display().to_string())
}

#[tauri::command(async)]
pub fn get_download_dir_settings(app: AppHandle) -> Result<DownloadDirectorySettings, String> {
    let default_dir = default_download_dir(&app)?;
    let settings = load_app_settings(&app)?;
    let current_dir = effective_download_dir(default_dir.clone(), settings.download_dir.as_deref());

    fs::create_dir_all(&current_dir).map_err(|err| format!("创建下载目录失败：{err}"))?;

    Ok(DownloadDirectorySettings {
        current_dir: current_dir.display().to_string(),
        default_dir: default_dir.display().to_string(),
        is_custom: settings
            .download_dir
            .as_deref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false),
    })
}

#[tauri::command(async)]
pub fn set_download_dir(app: AppHandle, path: String) -> Result<String, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("下载目录不能为空。".into());
    }

    let target = PathBuf::from(trimmed);
    fs::create_dir_all(&target).map_err(|err| format!("创建下载目录失败：{err}"))?;

    let mut settings = load_app_settings(&app)?;
    settings.download_dir = Some(target.display().to_string());
    save_app_settings(&app, &settings)?;
    Ok(target.display().to_string())
}

#[tauri::command(async)]
pub fn reset_download_dir(app: AppHandle) -> Result<String, String> {
    let mut settings = load_app_settings(&app)?;
    settings.download_dir = None;
    save_app_settings(&app, &settings)?;

    let default_dir = default_download_dir(&app)?;
    fs::create_dir_all(&default_dir).map_err(|err| format!("创建下载目录失败：{err}"))?;
    Ok(default_dir.display().to_string())
}

/// Downloads land in a dedicated `video-downloader` subfolder of the user's
/// Downloads directory (e.g. `C:\Users\<user>\Downloads\video-downloader`). The
/// opener scope in capabilities (`$DOWNLOAD/**`) must cover this for "打开下载目录".
const DOWNLOAD_SUBDIR: &str = "video-downloader";
const INCOMPLETE_SUBDIR: &str = "incomplete";

fn resolve_download_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let default_dir = default_download_dir(app)?;
    let settings = load_app_settings(app)?;
    Ok(effective_download_dir(
        default_dir,
        settings.download_dir.as_deref(),
    ))
}

fn run_ssstwitter_download_task(
    app: AppHandle,
    task_id: String,
    title: String,
    url: String,
    selection: SssTwitterSelection,
    download_dir: PathBuf,
    cancel_requested: Arc<AtomicBool>,
) -> Result<(), String> {
    let selection_label = selection.label.as_str();
    eprintln!(
        "[run_ssstwitter_download_task] task_id={task_id} selection_label={selection_label} direct_url_present={}",
        selection.direct_url.is_some()
    );
    let file_title = compose_download_title(&title, Some(selection_label));
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
    let slot = ssstwitter_download_slot();
    let wait_notice_needed = slot.try_lock().is_err();
    if wait_notice_needed {
        emit_status(
            &app,
            DownloadStatusPayload {
                task_id: task_id.clone(),
                title: title.clone(),
                status: "queued".into(),
                message: Some("ssstwitter 下载通道繁忙，正在排队等待。".into()),
                output_path: None,
            },
        );
    }
    let _slot_guard = slot
        .lock()
        .map_err(|_| "ssstwitter 下载通道状态异常。".to_string())?;
    if cancel_requested.load(Ordering::SeqCst) {
        return Err("下载已取消".into());
    }
    emit_status(
        &app,
        DownloadStatusPayload {
            task_id: task_id.clone(),
            title: title.clone(),
            status: "downloading".into(),
            message: Some(format!("开始通过 ssstwitter 下载：{selection_label}")),
            output_path: None,
        },
    );

    // ssstwitter only serves x.com videos, so this always lands in 视频.
    let output_path =
        category_dir(&download_dir, "mp4").join(format!("{}.{}", sanitize_filename(&file_title), "mp4"));
    // Stream into the system temp dir, then move into Downloads on completion.
    // Windows Defender scans the Downloads folder far more aggressively (downloaded-
    // file / mark-of-the-web handling), throttling the active write; temp is lighter,
    // and the final same-volume rename is an instant metadata op.
    let staging_path = staging_path_for(&download_dir, &task_id);
    if let Some(parent) = staging_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("创建临时下载目录失败：{err}"))?;
    }

    // Timing starts at the first received byte (not before), and speed is sampled
    // over the last interval so the UI shows real throughput instead of an average
    // that is dragged down by connection setup.
    let mut transfer_started_at: Option<Instant> = None;
    let mut last_emit_at = Instant::now() - Duration::from_secs(1);
    let mut last_sample: Option<(Instant, u64)> = None;
    let result = download_selection_to_path(
        &url,
        selection.direct_url.as_deref(),
        Some(selection_label),
        &staging_path,
        None,
        || cancel_requested.load(Ordering::SeqCst),
        |downloaded_bytes, total_bytes| {
            let now = Instant::now();
            let started_at = *transfer_started_at.get_or_insert(now);
            if last_emit_at.elapsed() >= Duration::from_millis(300) {
                let speed_bps = match last_sample {
                    Some((sample_at, sample_bytes)) => {
                        let dt = now.duration_since(sample_at).as_secs_f64().max(0.001);
                        (downloaded_bytes.saturating_sub(sample_bytes)) as f64 / dt
                    }
                    None => {
                        let dt = now.duration_since(started_at).as_secs_f64().max(0.001);
                        downloaded_bytes as f64 / dt
                    }
                };
                emit_progress(&app, task_id.as_str(), downloaded_bytes, total_bytes, speed_bps);
                last_emit_at = now;
                last_sample = Some((now, downloaded_bytes));
            }
        },
    );

    let result = match result {
        Ok(result) => result,
        Err(error) => return Err(error),
    };

    move_into_place(&staging_path, &output_path)?;

    let overall_bps = transfer_started_at
        .map(|started| result.downloaded_bytes as f64 / started.elapsed().as_secs_f64().max(0.001))
        .unwrap_or(0.0);
    emit_progress(
        &app,
        task_id.as_str(),
        result.downloaded_bytes,
        result.total_bytes,
        overall_bps,
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

fn ssstwitter_download_slot() -> &'static Mutex<()> {
    SSSTWITTER_DOWNLOAD_SLOT.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
fn with_ssstwitter_download_slot<F, T>(task: F) -> T
where
    F: FnOnce() -> T,
{
    let _guard = ssstwitter_download_slot()
        .lock()
        .expect("ssstwitter download slot should not be poisoned");
    task()
}

/// A staging file in the system temp dir for an in-progress download.
fn staging_path_for(download_dir: &Path, task_id: &str) -> PathBuf {
    download_dir
        .join(INCOMPLETE_SUBDIR)
        .join(format!("{task_id}.part"))
}

/// Move the finished staging file to its destination. A same-volume rename is an
/// instant metadata operation; if the destination is on another volume the rename
/// fails with a cross-device error, so we fall back to copy + delete.
fn move_into_place(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("创建下载目录失败：{err}"))?;
    }

    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(from, to).map_err(|err| format!("移动下载文件失败：{err}"))?;
            let _ = fs::remove_file(from);
            Ok(())
        }
    }
}

fn default_download_dir(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(path) = app.path().download_dir() {
        return Ok(path.join(DOWNLOAD_SUBDIR));
    }

    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    Ok(project_root.join("downloads").join(DOWNLOAD_SUBDIR))
}

fn effective_download_dir(default_dir: PathBuf, configured_dir: Option<&str>) -> PathBuf {
    configured_dir
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or(default_dir)
}

/// Per-website folder under the download base, e.g. `<base>/instagram.com`.
fn site_target_dir(base: &Path, url: &str) -> PathBuf {
    base.join(site_folder(url))
}

/// Top-level folder per website, derived from the host (without a `www.` prefix).
fn site_folder(url: &str) -> String {
    let host = reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_string))
        .unwrap_or_default();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    let cleaned = sanitize_filename(host);
    if cleaned.is_empty() || cleaned == "download" {
        "other".into()
    } else {
        cleaned
    }
}

/// Coarse resource-type folder inside a site folder, by file extension. All
/// videos land together, all images together, etc. — no per-post/author folders.
fn category_folder(ext: &str) -> &'static str {
    match ext.trim().trim_start_matches('.').to_ascii_lowercase().as_str() {
        "mp4" | "webm" | "mkv" | "mov" | "m4v" | "flv" | "avi" | "ts" | "3gp" => "视频",
        "jpg" | "jpeg" | "png" | "webp" | "gif" | "heic" | "heif" | "bmp" | "avif" => "图片",
        "m4a" | "mp3" | "aac" | "opus" | "ogg" | "oga" | "wav" | "flac" => "音频",
        _ => "其他",
    }
}

fn category_dir(site_dir: &Path, ext: &str) -> PathBuf {
    site_dir.join(category_folder(ext))
}

/// Move a finished file into its resource-type folder under `site_dir`, returning
/// the new path. Best-effort: on any failure the original path is returned.
fn relocate_into_category(file_path: &str, site_dir: &Path) -> String {
    let path = Path::new(file_path);
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let Some(file_name) = path.file_name() else {
        return file_path.to_string();
    };
    let dest = category_dir(site_dir, ext).join(file_name);
    if dest == path {
        return file_path.to_string();
    }
    match move_into_place(path, &dest) {
        Ok(()) => dest.display().to_string(),
        Err(_) => file_path.to_string(),
    }
}

/// Persistent config lives in a device-level folder under the user's home
/// directory (`~/.swell-video-downloader`) instead of the app's data dir, so the
/// config survives an app uninstall. `SWELL_CONFIG_DIR` overrides the location.
fn config_base_dir(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(dir) = std::env::var("SWELL_CONFIG_DIR") {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }

    let home = app
        .path()
        .home_dir()
        .map_err(|err| format!("读取用户主目录失败：{err}"))?;
    Ok(home.join(".swell-video-downloader"))
}

fn app_settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_base_dir(app)?.join("config.json"))
}

fn encode_secret(value: &str) -> String {
    STANDARD.encode(value.as_bytes())
}

fn decode_secret(value: &str) -> Option<String> {
    STANDARD
        .decode(value)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn settings_to_payload(settings: &AppSettings) -> AppSettingsPayload {
    AppSettingsPayload {
        cookie_source: settings.cookie_source.clone(),
        cookie_file_path: settings.cookie_file_path.clone(),
        instagram_sessionid: settings
            .instagram_sessionid_b64
            .as_deref()
            .and_then(decode_secret),
        instagram_cookie_file_path: settings.instagram_cookie_file_path.clone(),
        instagram_collect_mode: settings.instagram_collect_mode.clone(),
        instagram_collect_count: settings.instagram_collect_count.clone(),
        auto_download: settings.auto_download,
    }
}

#[tauri::command(async)]
pub fn get_app_settings(app: AppHandle) -> Result<AppSettingsPayload, String> {
    let settings = load_app_settings(&app)?;
    Ok(settings_to_payload(&settings))
}

#[tauri::command(async)]
pub fn set_app_settings(
    app: AppHandle,
    settings: AppSettingsUpdate,
) -> Result<AppSettingsPayload, String> {
    // Merge over the existing file so download_dir (managed separately) is kept.
    let mut stored = load_app_settings(&app)?;
    stored.cookie_source = normalize_optional(settings.cookie_source);
    stored.cookie_file_path = normalize_optional(settings.cookie_file_path);
    stored.instagram_cookie_file_path = normalize_optional(settings.instagram_cookie_file_path);
    stored.instagram_collect_mode = normalize_optional(settings.instagram_collect_mode);
    stored.instagram_collect_count = normalize_optional(settings.instagram_collect_count);
    stored.instagram_sessionid_b64 =
        normalize_optional(settings.instagram_sessionid).map(|value| encode_secret(&value));
    stored.auto_download = settings.auto_download;

    save_app_settings(&app, &stored)?;
    Ok(settings_to_payload(&stored))
}

fn load_app_settings(app: &AppHandle) -> Result<AppSettings, String> {
    let path = app_settings_path(app)?;
    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let contents = fs::read_to_string(&path).map_err(|err| format!("读取设置失败：{err}"))?;
    serde_json::from_str(&contents).map_err(|err| format!("解析设置失败：{err}"))
}

fn save_app_settings(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let path = app_settings_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("创建配置目录失败：{err}"))?;
    }

    let json = serde_json::to_string_pretty(settings).map_err(|err| format!("序列化设置失败：{err}"))?;
    fs::write(path, json).map_err(|err| format!("保存设置失败：{err}"))
}

fn direct_download_url(format_id: Option<&str>) -> Option<String> {
    format_id
        .map(str::trim)
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        .map(str::to_string)
}

fn apply_download_progress_args(command: &mut Command) {
    command.arg("--progress");
    command.arg("--newline");
    command.arg("--progress-template");
    command.arg(
        "download:__PROGRESS__:%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s",
    );
}

fn requires_binary_toolchain(url: &str, format_id: Option<&str>) -> bool {
    !(url.contains("x.com")
        && format_id
            .and_then(extract_ssstwitter_selection)
            .is_some())
}

fn compose_download_title(base_title: &str, format_label: Option<&str>) -> String {
    let base = base_title.trim();
    let descriptor = format_label.and_then(clean_format_descriptor);
    match descriptor {
        Some(value) if !base.contains(&value) => format!("{base} - {value}"),
        _ => base.to_string(),
    }
}

fn clean_format_descriptor(label: &str) -> Option<String> {
    let cleaned = label
        .trim()
        .strip_prefix("下载")
        .unwrap_or(label.trim())
        .trim();

    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.split_whitespace().collect::<Vec<_>>().join(" "))
    }
}

fn emit_progress(
    app: &AppHandle,
    task_id: &str,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    bytes_per_second: f64,
) {
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
    use super::{
        apply_download_progress_args,
        category_dir, category_folder, clean_format_descriptor, compose_download_title,
        decode_secret, direct_download_url, effective_download_dir, encode_secret,
        extract_ssstwitter_selection, format_eta, normalize_optional, requires_binary_toolchain,
        sanitize_filename, site_folder, site_target_dir, staging_path_for,
        with_ssstwitter_download_slot,
    };
    use std::{
        path::{Path, PathBuf},
        process::Command,
        sync::mpsc,
        thread,
        time::{Duration, Instant},
    };

    #[test]
    fn download_command_enables_progress_output() {
        let mut command = Command::new("yt-dlp");
        apply_download_progress_args(&mut command);

        let rendered = format!("{command:?}");
        assert!(rendered.contains("--progress"));
        assert!(rendered.contains("--progress-template"));
        assert!(rendered.contains("__PROGRESS__"));
    }

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
        let selection = extract_ssstwitter_selection("ssstwitter:下载 HD 1080x1080")
            .expect("selection should decode");
        assert_eq!(selection.label, "下载 HD 1080x1080");
        assert_eq!(selection.direct_url, None);
    }

    #[test]
    fn skips_binary_toolchain_for_x_ssstwitter_downloads() {
        assert!(!requires_binary_toolchain(
            "https://x.com/4Brazzerlive/status/2068062512831255010/video/1",
            Some("ssstwitter:eyJsYWJlbCI6IuS4i-i9vSBIRCAxMDgweDEwODAiLCJkaXJlY3RfdXJsIjoiaHR0cHM6Ly9leGFtcGxlLmNvbS92aWRlby5tcDQifQ")
        ));
        assert!(requires_binary_toolchain(
            "https://www.pornhub.com/view_video.php?viewkey=abc123",
            Some("best")
        ));
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

    #[test]
    fn prefers_custom_download_dir_when_configured() {
        let dir = effective_download_dir(
            PathBuf::from(r"C:\Users\Administrator\Downloads\video-downloader"),
            Some(r"D:\Media\Twitter"),
        );

        assert_eq!(dir, PathBuf::from(r"D:\Media\Twitter"));
    }

    #[test]
    fn keeps_default_download_dir_when_override_is_blank() {
        let default_dir = PathBuf::from(r"C:\Users\Administrator\Downloads\video-downloader");
        let dir = effective_download_dir(default_dir.clone(), Some("   "));

        assert_eq!(dir, default_dir);
    }

    #[test]
    fn sessionid_secret_round_trips_through_base64() {
        let encoded = encode_secret("abc123:sessionid");
        assert_ne!(encoded, "abc123:sessionid");
        assert_eq!(decode_secret(&encoded).as_deref(), Some("abc123:sessionid"));
    }

    #[test]
    fn normalize_optional_drops_blank_values() {
        assert_eq!(normalize_optional(Some("  hi  ".into())).as_deref(), Some("hi"));
        assert_eq!(normalize_optional(Some("   ".into())), None);
        assert_eq!(normalize_optional(None), None);
    }

    #[test]
    fn site_folder_uses_host_without_www_prefix() {
        assert_eq!(
            site_folder("https://www.instagram.com/p/Abc123/"),
            "instagram.com"
        );
        assert_eq!(site_folder("https://x.com/user/status/42"), "x.com");
        assert_eq!(site_folder("not a url"), "other");
    }

    #[test]
    fn category_folder_groups_by_resource_type() {
        assert_eq!(category_folder("mp4"), "视频");
        assert_eq!(category_folder("MOV"), "视频");
        assert_eq!(category_folder(".webm"), "视频");
        assert_eq!(category_folder("jpg"), "图片");
        assert_eq!(category_folder("webp"), "图片");
        assert_eq!(category_folder("m4a"), "音频");
        assert_eq!(category_folder("mp3"), "音频");
        assert_eq!(category_folder("srt"), "其他");
        assert_eq!(category_folder(""), "其他");
    }

    #[test]
    fn site_and_category_dirs_nest_under_base() {
        let site = site_target_dir(
            Path::new(r"C:\Downloads\video-downloader"),
            "https://www.instagram.com/reels/DZxVqbsTzqZ/",
        );
        assert_eq!(
            site,
            PathBuf::from(r"C:\Downloads\video-downloader\instagram.com")
        );

        assert_eq!(
            category_dir(&site, "mp4"),
            PathBuf::from(r"C:\Downloads\video-downloader\instagram.com\视频")
        );
    }

    #[test]
    fn stores_partial_files_inside_visible_incomplete_folder() {
        let path = staging_path_for(Path::new(r"C:\Downloads\video-downloader"), "task-42");

        assert_eq!(
            path,
            PathBuf::from(r"C:\Downloads\video-downloader\incomplete\task-42.part")
        );
    }

    #[test]
    fn strips_download_prefix_from_quality_labels() {
        assert_eq!(
            clean_format_descriptor("下载 HD 1080x1080").as_deref(),
            Some("HD 1080x1080")
        );
        assert_eq!(clean_format_descriptor("  下载 720p ").as_deref(), Some("720p"));
    }

    #[test]
    fn avoids_duplicate_quality_suffix_when_title_already_contains_it() {
        assert_eq!(
            compose_download_title("@4Brazzerlive 的 X 视频 - HD 1080x1080", Some("下载 HD 1080x1080")),
            "@4Brazzerlive 的 X 视频 - HD 1080x1080"
        );
    }

    #[test]
    fn appends_clean_quality_once_when_missing_from_title() {
        assert_eq!(
            compose_download_title("@4Brazzerlive 的 X 视频", Some("下载 HD 1080x1080")),
            "@4Brazzerlive 的 X 视频 - HD 1080x1080"
        );
    }

    #[test]
    fn serializes_ssstwitter_downloads_through_single_slot() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        let holder = thread::spawn(move || {
            with_ssstwitter_download_slot(|| {
                entered_tx.send(()).expect("holder should report entry");
                release_rx.recv().expect("holder should wait for release");
            });
        });

        entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("holder should enter slot first");

        let started_at = Instant::now();
        let waiter = thread::spawn(move || with_ssstwitter_download_slot(|| Instant::now()));

        thread::sleep(Duration::from_millis(150));
        release_tx.send(()).expect("release signal should be sent");

        let entered_at = waiter.join().expect("waiter should complete");
        holder.join().expect("holder should complete");

        assert!(
            entered_at.duration_since(started_at) >= Duration::from_millis(140),
            "second download should wait for the first ssstwitter slot"
        );
    }
}
