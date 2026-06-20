use crate::downloader::yt_dlp::fetch_metadata;
use crate::downloader::x_probe::probe_x_guest_restriction;
use crate::downloader::x_ssstwitter::{create_ssstwitter_selection_id, resolve_x_via_ssstwitter};
use serde::Serialize;
use tauri::AppHandle;

#[derive(Clone, Serialize)]
pub struct MediaFormat {
    pub id: String,
    pub label: String,
    pub ext: String,
    pub has_audio: bool,
    pub note: String,
}

#[derive(Serialize)]
pub struct ResolveMediaResponse {
    pub title: String,
    pub source: String,
    pub duration_text: String,
    pub recommendation: MediaFormat,
    pub formats: Vec<MediaFormat>,
}

#[tauri::command]
pub fn resolve_media(
    app: AppHandle,
    url: String,
    cookie_source: Option<String>,
    cookie_file_path: Option<String>,
) -> Result<ResolveMediaResponse, String> {
    if !(url.contains("x.com") || url.contains("pornhub.com")) {
        return Err("仅支持 x.com 和 pornhub.com".into());
    }

    let source = if url.contains("x.com") {
        "x.com".to_string()
    } else {
        "pornhub.com".to_string()
    };
    let metadata = match fetch_metadata(
        &app,
        &url,
        cookie_source.as_deref(),
        cookie_file_path.as_deref(),
    ) {
        Ok(metadata) => metadata,
        Err(error) if source == "x.com" => {
            if let Ok(fallback_formats) = resolve_x_via_ssstwitter(&url) {
                let recommendation = fallback_formats.first().cloned().unwrap();
                let formats = fallback_formats
                    .into_iter()
                    .map(|format| MediaFormat {
                        id: create_ssstwitter_selection_id(&format.label),
                        label: format.label,
                        ext: format.ext,
                        has_audio: true,
                        note: format.note,
                    })
                    .collect::<Vec<_>>();

                return Ok(ResolveMediaResponse {
                    title: "X 视频（ssstwitter 回退）".into(),
                    source,
                    duration_text: "--:--".into(),
                    recommendation: MediaFormat {
                        id: create_ssstwitter_selection_id(&recommendation.label),
                        label: recommendation.label,
                        ext: recommendation.ext,
                        has_audio: true,
                        note: recommendation.note,
                    },
                    formats,
                });
            }

            if let Ok(Some(restriction)) = probe_x_guest_restriction(&url) {
                let author_hint = restriction
                    .author_screen_name
                    .map(|screen_name| format!(" 账号：@{screen_name}。"))
                    .unwrap_or_default();
                return Err(format!(
                    "X 访客接口当前只返回成人内容遮罩，无法直接拿到真实视频资源。{author_hint}请改用登录态 Cookie 或后续的 cookies.txt 导入链路。原始提示：{}",
                    restriction.message
                ));
            }

            return Err(error);
        }
        Err(error) => return Err(error),
    };

    let title = metadata.title.unwrap_or_else(|| "未识别标题".into());
    let duration_text = metadata
        .duration
        .map(format_duration)
        .unwrap_or_else(|| "--:--".into());
    let formats = metadata
        .formats
        .unwrap_or_default()
        .into_iter()
        .filter_map(|format| {
            let id = format.format_id?;
            let ext = format.ext.unwrap_or_else(|| "unknown".into());
            let label = format
                .height
                .map(|height| format!("{height}p"))
                .unwrap_or_else(|| id.clone());
            let note = format.format_note.unwrap_or_else(|| "来自 yt-dlp".into());

            Some(MediaFormat {
                id,
                label,
                ext,
                has_audio: format.acodec.as_deref().unwrap_or("none") != "none",
                note,
            })
        })
        .collect::<Vec<_>>();
    let recommendation = formats.first().cloned().unwrap_or(MediaFormat {
        id: "best".into(),
        label: "best".into(),
        ext: "mp4".into(),
        has_audio: true,
        note: "来自 yt-dlp".into(),
    });

    Ok(ResolveMediaResponse {
        title,
        source,
        duration_text,
        recommendation,
        formats,
    })
}

fn format_duration(value: f64) -> String {
    let total_seconds = value.round() as u64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}
