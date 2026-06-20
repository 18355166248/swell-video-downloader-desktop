use serde::Serialize;
use tauri::AppHandle;

use crate::platform::binaries::{resolve_ffmpeg, resolve_yt_dlp};

#[derive(Serialize)]
pub struct DependencyStatus {
    pub yt_dlp_ok: bool,
    pub ffmpeg_ok: bool,
    pub yt_dlp_source: String,
    pub ffmpeg_source: String,
}

#[tauri::command]
pub fn check_dependencies(app: AppHandle) -> DependencyStatus {
    let yt_dlp = resolve_yt_dlp(&app);
    let ffmpeg = resolve_ffmpeg(&app);

    DependencyStatus {
        yt_dlp_ok: yt_dlp.is_some(),
        ffmpeg_ok: ffmpeg.is_some(),
        yt_dlp_source: yt_dlp
            .map(|item| item.source.to_string())
            .unwrap_or_else(|| "missing".into()),
        ffmpeg_source: ffmpeg
            .map(|item| item.source.to_string())
            .unwrap_or_else(|| "missing".into()),
    }
}
