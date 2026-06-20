use crate::downloader::page_title::fetch_page_title;
use crate::downloader::yt_dlp::{
    fetch_metadata, probe_metadata, summarize_formats, DiagnosticCommandPreview, FormatSummary,
};
use crate::downloader::x_probe::probe_x_guest_restriction;
use crate::downloader::x_ssstwitter::{create_ssstwitter_selection_id, resolve_x_via_ssstwitter};
use crate::platform::binaries::resolve_ffmpeg;
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

#[derive(Serialize)]
pub struct DiagnoseMediaResponse {
    pub resolved: Option<ResolveMediaResponse>,
    pub diagnostics: MediaDiagnostics,
}

#[derive(Serialize)]
pub struct MediaDiagnostics {
    pub cookie_mode: String,
    pub yt_dlp_source: String,
    pub ffmpeg_source: String,
    pub proxy_enabled: bool,
    pub command_preview: DiagnosticCommandPreview,
    pub formats_count: usize,
    pub best_format_id: Option<String>,
    pub best_height: Option<u64>,
    pub max_height: Option<u64>,
    pub best_has_audio: bool,
    pub has_muxed_format: bool,
    pub has_video_only_format: bool,
    pub has_audio_only_format: bool,
    pub error_category: Option<String>,
    pub normalized_message: Option<String>,
    pub raw_error_message: Option<String>,
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

#[tauri::command]
pub async fn diagnose_media(
    app: AppHandle,
    url: String,
    cookie_source: Option<String>,
    cookie_file_path: Option<String>,
) -> Result<DiagnoseMediaResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        diagnose_media_blocking(app, url, cookie_source, cookie_file_path)
    })
    .await
    .map_err(|error| format!("诊断任务执行失败：{error}"))?
}

fn resolve_media_blocking(
    app: AppHandle,
    url: String,
    cookie_source: Option<String>,
    cookie_file_path: Option<String>,
) -> Result<ResolveMediaResponse, String> {
    let source = resolve_source(&url)?;
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

    Ok(build_resolve_response(&url, &source, html_title.as_deref(), metadata))
}

fn diagnose_media_blocking(
    app: AppHandle,
    url: String,
    cookie_source: Option<String>,
    cookie_file_path: Option<String>,
) -> Result<DiagnoseMediaResponse, String> {
    let source = resolve_source(&url)?;
    let html_title = fetch_page_title(&url);
    let ffmpeg_source = resolve_ffmpeg(&app)
        .map(|binary| binary.source.to_string())
        .unwrap_or_else(|| "missing".into());

    match probe_metadata(
        &app,
        &url,
        cookie_source.as_deref(),
        cookie_file_path.as_deref(),
    ) {
        Ok(success) => {
            let recommendation_id = success
                .metadata
                .formats
                .as_deref()
                .and_then(|formats| {
                    formats
                        .iter()
                        .filter_map(|format| {
                            let id = format.format_id.as_deref()?;
                            let height = format.height.unwrap_or(0);
                            let has_audio = format.acodec.as_deref().unwrap_or("none") != "none";
                            Some((id, height, has_audio, format.filesize.or(format.filesize_approx).unwrap_or(0)))
                        })
                        .max_by(|left, right| {
                            left.1
                                .cmp(&right.1)
                                .then_with(|| left.2.cmp(&right.2))
                                .then_with(|| left.3.cmp(&right.3))
                        })
                        .map(|entry| entry.0)
                })
                .unwrap_or("best")
                .to_string();
            let summary = summarize_formats(
                recommendation_id.as_str(),
                success
                    .metadata
                    .formats
                    .as_deref()
                    .unwrap_or(&[]),
            );
            let resolved =
                build_resolve_response(&url, &source, html_title.as_deref(), success.metadata);

            Ok(DiagnoseMediaResponse {
                resolved: Some(resolved),
                diagnostics: diagnostics_from_summary(
                    success.cookie_mode,
                    success.yt_dlp_source,
                    ffmpeg_source,
                    success.proxy_enabled,
                    success.command_preview,
                    summary,
                    None,
                    None,
                    None,
                ),
            })
        }
        Err(failure) => Ok(DiagnoseMediaResponse {
            resolved: None,
            diagnostics: diagnostics_from_summary(
                failure.cookie_mode,
                failure.yt_dlp_source,
                ffmpeg_source,
                failure.proxy_enabled,
                failure.command_preview,
                FormatSummary {
                    formats_count: 0,
                    best_format_id: None,
                    best_height: None,
                    max_height: None,
                    best_has_audio: false,
                    has_muxed_format: false,
                    has_video_only_format: false,
                    has_audio_only_format: false,
                },
                Some(failure.error_info.error_category),
                Some(failure.error_info.normalized_message),
                Some(failure.raw_message),
            ),
        }),
    }
}

fn resolve_source(url: &str) -> Result<String, String> {
    if !(url.contains("x.com") || url.contains("pornhub.com")) {
        return Err("仅支持 x.com 和 pornhub.com".into());
    }

    Ok(if url.contains("x.com") {
        "x.com".to_string()
    } else {
        "pornhub.com".to_string()
    })
}

fn build_resolve_response(
    url: &str,
    source: &str,
    html_title: Option<&str>,
    metadata: crate::downloader::yt_dlp::YtDlpMetadata,
) -> ResolveMediaResponse {
    let title = pick_best_title(metadata.title.as_deref(), html_title, url, source);
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
    let recommendation = pick_recommended_format(&formats).unwrap_or(MediaFormat {
        id: "best".into(),
        label: "best".into(),
        ext: "mp4".into(),
        has_audio: true,
        note: "来自 yt-dlp".into(),
        size_bytes: None,
    });

    ResolveMediaResponse {
        title,
        source: source.to_string(),
        duration_text,
        recommendation,
        formats,
        thumbnail: metadata.thumbnail,
    }
}

fn pick_recommended_format(formats: &[MediaFormat]) -> Option<MediaFormat> {
    formats
        .iter()
        .max_by(|left, right| {
            let left_height = parse_format_height(&left.label);
            let right_height = parse_format_height(&right.label);

            left_height
                .cmp(&right_height)
                .then_with(|| left.has_audio.cmp(&right.has_audio))
                .then_with(|| left.size_bytes.unwrap_or(0).cmp(&right.size_bytes.unwrap_or(0)))
        })
        .cloned()
}

fn parse_format_height(label: &str) -> u64 {
    label
        .strip_suffix('p')
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0)
}

fn diagnostics_from_summary(
    cookie_mode: String,
    yt_dlp_source: String,
    ffmpeg_source: String,
    proxy_enabled: bool,
    command_preview: DiagnosticCommandPreview,
    summary: FormatSummary,
    error_category: Option<String>,
    normalized_message: Option<String>,
    raw_error_message: Option<String>,
) -> MediaDiagnostics {
    MediaDiagnostics {
        cookie_mode,
        yt_dlp_source,
        ffmpeg_source,
        proxy_enabled,
        command_preview,
        formats_count: summary.formats_count,
        best_format_id: summary.best_format_id,
        best_height: summary.best_height,
        max_height: summary.max_height,
        best_has_audio: summary.best_has_audio,
        has_muxed_format: summary.has_muxed_format,
        has_video_only_format: summary.has_video_only_format,
        has_audio_only_format: summary.has_audio_only_format,
        error_category,
        normalized_message,
        raw_error_message,
    }
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
    use super::{
        fallback_title_from_url, pick_best_title, pick_recommended_format, resolve_source,
        MediaFormat,
    };

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

    #[test]
    fn rejects_unsupported_hosts_from_resolve_and_diagnose_paths() {
        let error = resolve_source("https://example.com/video/123").expect_err("host should reject");

        assert!(error.contains("仅支持"));
    }

    #[test]
    fn recommends_highest_quality_over_first_format() {
        let recommendation = pick_recommended_format(&[
            MediaFormat {
                id: "low".into(),
                label: "240p".into(),
                ext: "mp4".into(),
                has_audio: false,
                note: "low".into(),
                size_bytes: Some(10),
            },
            MediaFormat {
                id: "best".into(),
                label: "1080p".into(),
                ext: "mp4".into(),
                has_audio: false,
                note: "best".into(),
                size_bytes: Some(20),
            },
        ])
        .expect("recommendation should exist");

        assert_eq!(recommendation.id, "best");
        assert_eq!(recommendation.label, "1080p");
    }
}
