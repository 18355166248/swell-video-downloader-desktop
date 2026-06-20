use crate::downloader::page_title::fetch_page_title;
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
    pub size_bytes: Option<u64>,
}

#[derive(Serialize)]
pub struct ResolveMediaResponse {
    pub title: String,
    pub source: String,
    pub duration_text: String,
    pub recommendation: MediaFormat,
    pub formats: Vec<MediaFormat>,
    pub thumbnail: Option<String>,
}

// The work is synchronous and blocking (reqwest::blocking + yt-dlp). It must run on
// a blocking-friendly thread via `spawn_blocking` — running it directly on the async
// runtime panics when the reqwest client's internal tokio runtime is dropped
// ("Cannot drop a runtime in a context where blocking is not allowed"), and running
// it on the main thread freezes the window.
#[tauri::command]
pub async fn resolve_media(
    app: AppHandle,
    url: String,
    cookie_source: Option<String>,
    cookie_file_path: Option<String>,
) -> Result<ResolveMediaResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        resolve_media_blocking(app, url, cookie_source, cookie_file_path)
    })
    .await
    .map_err(|error| format!("解析任务执行失败：{error}"))?
}

fn resolve_media_blocking(
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
    let html_title = fetch_page_title(&url);
    let metadata = match fetch_metadata(
        &app,
        &url,
        cookie_source.as_deref(),
        cookie_file_path.as_deref(),
    ) {
        Ok(metadata) => metadata,
        Err(error) if source == "x.com" => {
            if let Ok(fallback_formats) = resolve_x_via_ssstwitter(&url) {
                let formats = fallback_formats
                    .into_iter()
                    .map(|format| MediaFormat {
                        id: create_ssstwitter_selection_id(
                            &format.label,
                            Some(&format.download_url),
                        ),
                        label: format.label,
                        ext: format.ext,
                        has_audio: true,
                        note: format.note,
                        size_bytes: format.size_bytes,
                    })
                    .collect::<Vec<_>>();
                let recommendation = formats.first().cloned().unwrap();

                return Ok(ResolveMediaResponse {
                    title: pick_best_title(None, html_title.as_deref(), &url, &source),
                    source,
                    duration_text: "--:--".into(),
                    recommendation,
                    formats,
                    thumbnail: None,
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

    let title = pick_best_title(metadata.title.as_deref(), html_title.as_deref(), &url, &source);
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
                size_bytes: format.filesize.or(format.filesize_approx),
            })
        })
        .collect::<Vec<_>>();
    let recommendation = formats.first().cloned().unwrap_or(MediaFormat {
        id: "best".into(),
        label: "best".into(),
        ext: "mp4".into(),
        has_audio: true,
        note: "来自 yt-dlp".into(),
        size_bytes: None,
    });

    Ok(ResolveMediaResponse {
        title,
        source,
        duration_text,
        recommendation,
        formats,
        thumbnail: metadata.thumbnail,
    })
}

fn pick_best_title(
    metadata_title: Option<&str>,
    html_title: Option<&str>,
    url: &str,
    source: &str,
) -> String {
    metadata_title
        .and_then(clean_title)
        .filter(|title| !is_low_signal_title(title, source))
        .or_else(|| {
            html_title
                .and_then(clean_title)
                .filter(|title| !is_low_signal_title(title, source))
        })
        .or_else(|| fallback_title_from_url(url, source))
        .unwrap_or_else(|| "未识别标题".into())
}

fn clean_title(value: &str) -> Option<String> {
    let title = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = title.trim_matches(|ch: char| ch == '-' || ch == '|' || ch.is_whitespace());
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn fallback_title_from_url(url: &str, source: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    if source == "x.com" {
        let segments = parsed
            .path_segments()
            .map(|items| items.filter(|item| !item.is_empty()).collect::<Vec<_>>())
            .unwrap_or_default();
        if segments.len() >= 3 && segments.get(1) == Some(&"status") {
            return Some(format!("@{} - {}", segments[0], segments[2]));
        }
        if let Some(user) = segments.first() {
            return Some(format!("@{user} 的 X 视频"));
        }
        return Some("X 视频".into());
    }

    let slug = parsed
        .path_segments()
        .and_then(|segments| segments.filter(|item| !item.is_empty()).next_back())
        .filter(|segment| !segment.contains('.'))
        .map(|segment| segment.replace('-', " "))
        .and_then(|segment| clean_title(&segment));

    slug.or_else(|| Some(source.to_string()))
}

fn is_low_signal_title(title: &str, source: &str) -> bool {
    let normalized = title.trim().to_ascii_lowercase();
    match source {
        "x.com" => matches!(normalized.as_str(), "x" | "twitter" | "x.com"),
        "pornhub.com" => matches!(normalized.as_str(), "pornhub" | "pornhub.com"),
        _ => false,
    }
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

#[cfg(test)]
mod tests {
    use super::{fallback_title_from_url, pick_best_title};

    #[test]
    fn prefers_metadata_title_over_html_title() {
        let title = pick_best_title(
            Some("解析标题"),
            Some("页面标题"),
            "https://x.com/demo/status/123",
            "x.com",
        );

        assert_eq!(title, "解析标题");
    }

    #[test]
    fn uses_html_title_when_metadata_missing() {
        let title = pick_best_title(
            None,
            Some("页面返回标题"),
            "https://x.com/demo/status/123",
            "x.com",
        );

        assert_eq!(title, "页面返回标题");
    }

    #[test]
    fn falls_back_to_username_or_slug_from_url() {
        assert_eq!(
            fallback_title_from_url("https://x.com/demo_user/status/123", "x.com").as_deref(),
            Some("@demo_user - 123")
        );
        assert_eq!(
            fallback_title_from_url(
                "https://www.pornhub.com/view_video.php?viewkey=abc123",
                "pornhub.com"
            )
            .as_deref(),
            Some("pornhub.com")
        );
    }

    #[test]
    fn ignores_low_signal_page_titles_and_uses_status_identifier() {
        let title = pick_best_title(
            None,
            Some("X"),
            "https://x.com/4Brazzerlive/status/2068239068916507115/video/1",
            "x.com",
        );

        assert_eq!(title, "@4Brazzerlive - 2068239068916507115");
    }
}
