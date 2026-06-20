use reqwest::blocking::{Client, Response};
use reqwest::header::{
    ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT,
};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
    time::Duration,
};

pub const SSSTWITTER_SELECTION_PREFIX: &str = "ssstwitter:";

/// Connecting must not hang forever — a stuck TCP/TLS handshake would otherwise
/// block the resolve command indefinitely (a frozen, "未响应" window).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Total timeout for the small resolve-phase requests (homepage, parse POST, size
/// probes). Not applied to the actual download, which streams large files.
const RESOLVE_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SssTwitterFormValues {
    pub action_path: String,
    pub tt: String,
    pub ts: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SssTwitterFormat {
    pub download_url: String,
    pub label: String,
    pub ext: String,
    pub note: String,
    pub size_bytes: Option<u64>,
}

/// A user-picked ssstwitter download option, decoded from the selection id that
/// the resolve step handed to the frontend. `direct_url` is the already-resolved
/// signed CDN link so the download step can stream it immediately instead of
/// re-querying ssstwitter (which adds ~6s of dead time before the first byte).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SssTwitterSelection {
    pub label: String,
    pub direct_url: Option<String>,
}

/// Field separator embedded in a selection id (ASCII unit separator) — it never
/// appears in a quality label or an https URL, so it round-trips losslessly.
const SELECTION_FIELD_SEPARATOR: char = '\u{1f}';

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SssTwitterDownloadResult {
    pub selected_format: SssTwitterFormat,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
}

pub fn resolve_x_via_ssstwitter(url: &str) -> Result<Vec<SssTwitterFormat>, String> {
    let client = create_client()?;
    let mut formats = fetch_formats_with_client(&client, url)?;
    fill_sizes(&client, &mut formats);
    Ok(formats)
}

/// Best-effort: learn each variant's size so the UI can show how big each quality
/// is before the user commits. The CDN ignores both HEAD and `Range`, so we issue
/// a normal GET, read `Content-Length` from the response headers, and drop the body
/// without downloading it. The GETs run concurrently to keep resolve snappy (each
/// has ~2s time-to-first-byte). Failures are ignored — a missing size renders as "未知".
fn fill_sizes(client: &Client, formats: &mut [SssTwitterFormat]) {
    let headers = match download_headers() {
        Ok(headers) => headers,
        Err(_) => return,
    };

    let handles = formats
        .iter()
        .map(|format| {
            let client = client.clone();
            let headers = headers.clone();
            let url = format.download_url.clone();
            std::thread::spawn(move || {
                client
                    .get(&url)
                    .timeout(RESOLVE_REQUEST_TIMEOUT)
                    .headers(headers)
                    .send()
                    .ok()
                    .filter(|response| response.status().is_success())
                    .and_then(|response| response.content_length())
                    .filter(|value| *value > 0)
            })
        })
        .collect::<Vec<_>>();

    for (format, handle) in formats.iter_mut().zip(handles) {
        if let Ok(size) = handle.join() {
            format.size_bytes = size;
        }
    }
}

pub fn create_ssstwitter_selection_id(label: &str, direct_url: Option<&str>) -> String {
    match direct_url.map(str::trim).filter(|value| !value.is_empty()) {
        Some(url) => format!(
            "{SSSTWITTER_SELECTION_PREFIX}{}{SELECTION_FIELD_SEPARATOR}{}",
            label.trim(),
            url
        ),
        None => format!("{SSSTWITTER_SELECTION_PREFIX}{}", label.trim()),
    }
}

pub fn extract_ssstwitter_selection(selection_id: &str) -> Option<SssTwitterSelection> {
    let body = selection_id.strip_prefix(SSSTWITTER_SELECTION_PREFIX)?;
    let mut parts = body.splitn(2, SELECTION_FIELD_SEPARATOR);
    let label = parts.next()?.trim();
    if label.is_empty() {
        return None;
    }
    let direct_url = parts
        .next()
        .map(str::trim)
        .filter(|value| value.starts_with("https://"))
        .map(str::to_string);

    Some(SssTwitterSelection {
        label: label.to_string(),
        direct_url,
    })
}

fn pick_format(
    formats: &[SssTwitterFormat],
    preferred_label: Option<&str>,
) -> Result<SssTwitterFormat, String> {
    let preferred_label = preferred_label.map(str::trim).filter(|value| !value.is_empty());
    preferred_label
        .and_then(|label| formats.iter().find(|format| format.label == label).cloned())
        .or_else(|| formats.first().cloned())
        .ok_or_else(|| "ssstwitter 未返回可下载格式。".to_string())
}

/// GET a signed CDN link. The link does not need the resolving session's cookies,
/// but it *does* require the ssstwitter referer/origin headers — without them the
/// CDN answers `204 No Content` (X-CDN-Error: 093).
fn open_direct_download(client: &Client, download_url: &str) -> Result<Response, String> {
    let response = client
        .get(download_url)
        .headers(download_headers()?)
        .send()
        .map_err(|error| format!("请求 ssstwitter 下载地址失败：{error}"))?;

    if !response.status().is_success() {
        return Err(format!("ssstwitter 下载地址返回异常：{}", response.status()));
    }

    if response.content_length() == Some(0) {
        return Err("ssstwitter 返回了空文件响应，当前下载链接可能已失效。".into());
    }

    Ok(response)
}

pub fn open_x_via_ssstwitter_download(
    url: &str,
    preferred_label: Option<&str>,
) -> Result<(Response, SssTwitterFormat), String> {
    let client = create_client()?;
    let formats = fetch_formats_with_client(&client, url)?;
    let selected = pick_format(&formats, preferred_label)?;
    let response = open_direct_download(&client, &selected.download_url)?;
    Ok((response, selected))
}

fn format_from_direct_url(download_url: &str, label: Option<&str>) -> SssTwitterFormat {
    let ext = if download_url.contains(".mp3") { "mp3" } else { "mp4" }.to_string();
    let label = label.map(str::trim).filter(|value| !value.is_empty()).unwrap_or("下载");
    SssTwitterFormat {
        download_url: download_url.to_string(),
        label: label.to_string(),
        ext,
        note: format!("来自 ssstwitter 回退：{label}"),
        size_bytes: None,
    }
}

fn stream_response_to_file<F, C>(
    mut response: Response,
    output_path: &Path,
    max_bytes: Option<u64>,
    should_cancel: C,
    mut on_progress: F,
) -> Result<(u64, Option<u64>), String>
where
    F: FnMut(u64, Option<u64>),
    C: Fn() -> bool,
{
    let total_bytes = response.content_length();
    let mut downloaded_bytes = 0u64;
    let file = File::create(output_path).map_err(|error| format!("创建输出文件失败：{error}"))?;
    // Buffer writes into ~1 MiB chunks. Each `write` syscall is intercepted by the
    // Windows Defender filter driver, so coalescing 256 KiB reads into large writes
    // dramatically cuts that per-write scanning overhead on slow real-time-scanned
    // destinations (e.g. the Downloads folder).
    let mut file = std::io::BufWriter::with_capacity(1024 * 1024, file);
    let mut buffer = vec![0u8; 256 * 1024];

    loop {
        if should_cancel() {
            return Err("下载已取消".into());
        }
        let read = response
            .read(&mut buffer)
            .map_err(|error| format!("读取 ssstwitter 下载流失败：{error}"))?;
        if read == 0 {
            break;
        }

        let bytes_to_write = if let Some(limit) = max_bytes {
            let remaining = limit.saturating_sub(downloaded_bytes);
            if remaining == 0 {
                break;
            }
            remaining.min(read as u64) as usize
        } else {
            read
        };

        file.write_all(&buffer[..bytes_to_write])
            .map_err(|error| format!("写入输出文件失败：{error}"))?;
        downloaded_bytes += bytes_to_write as u64;
        on_progress(downloaded_bytes, total_bytes);
        if should_cancel() {
            return Err("下载已取消".into());
        }

        if let Some(limit) = max_bytes {
            if downloaded_bytes >= limit {
                break;
            }
        }
    }

    file.flush().map_err(|error| format!("刷新输出文件失败：{error}"))?;

    Ok((downloaded_bytes, total_bytes))
}

/// Download a previously-resolved ssstwitter selection. When `direct_url` is
/// present we stream it straight away (no re-query, no ~6s stall); if that link
/// has expired we transparently re-resolve via `page_url` + label.
pub fn download_selection_to_path<F, C>(
    page_url: &str,
    direct_url: Option<&str>,
    preferred_label: Option<&str>,
    output_path: &Path,
    max_bytes: Option<u64>,
    should_cancel: C,
    mut on_progress: F,
) -> Result<SssTwitterDownloadResult, String>
where
    F: FnMut(u64, Option<u64>),
    C: Fn() -> bool,
{
    let client = create_client()?;

    if let Some(direct) = direct_url.map(str::trim).filter(|value| value.starts_with("https://")) {
        if let Ok(response) = open_direct_download(&client, direct) {
            let selected_format = format_from_direct_url(direct, preferred_label);
            let (downloaded_bytes, total_bytes) =
                stream_response_to_file(response, output_path, max_bytes, &should_cancel, &mut on_progress)?;
            return Ok(SssTwitterDownloadResult {
                selected_format,
                downloaded_bytes,
                total_bytes,
            });
        }
    }

    // The stored link expired (or was absent) — re-resolve and try again.
    let (response, selected_format) = open_x_via_ssstwitter_download(page_url, preferred_label)?;
    let (downloaded_bytes, total_bytes) =
        stream_response_to_file(response, output_path, max_bytes, should_cancel, &mut on_progress)?;
    Ok(SssTwitterDownloadResult {
        selected_format,
        downloaded_bytes,
        total_bytes,
    })
}

#[cfg(test)]
pub fn download_x_via_ssstwitter_to_path<F>(
    url: &str,
    preferred_label: Option<&str>,
    output_path: &Path,
    max_bytes: Option<u64>,
    on_progress: F,
) -> Result<SssTwitterDownloadResult, String>
where
    F: FnMut(u64, Option<u64>),
{
    download_selection_to_path(
        url,
        None,
        preferred_label,
        output_path,
        max_bytes,
        || false,
        on_progress,
    )
}

fn create_client() -> Result<Client, String> {
    let mut builder = Client::builder()
        .cookie_store(true)
        .connect_timeout(CONNECT_TIMEOUT)
        .tcp_keepalive(Duration::from_secs(20));

    // Route through the system proxy when present. reqwest reads proxy env vars on
    // its own but not the Windows system proxy, so without this the app connects
    // directly to the CDN — which is painfully slow for overseas hosts behind a
    // local proxy (Clash/V2Ray), where direct ~15 KiB/s vs proxied >1 MiB/s.
    if let Some(proxy_url) = crate::platform::proxy::detect_proxy() {
        if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
            builder = builder.proxy(proxy);
        }
    }

    builder
        .build()
        .map_err(|error| format!("创建 ssstwitter 请求客户端失败：{error}"))
}

/// Number of times to retry the full homepage+parse flow. ssstwitter intermittently
/// answers the parse POST with `200 OK` and an empty body (anti-bot / rate limiting),
/// which would otherwise surface as "未返回可下载格式". Re-fetching fresh form tokens
/// and retrying clears it the vast majority of the time.
const FETCH_FORMATS_MAX_ATTEMPTS: u32 = 4;

fn fetch_formats_with_client(client: &Client, url: &str) -> Result<Vec<SssTwitterFormat>, String> {
    let mut last_error =
        "ssstwitter 未返回可下载格式。".to_string();

    for attempt in 0..FETCH_FORMATS_MAX_ATTEMPTS {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(400 * attempt as u64));
        }

        match fetch_formats_once(client, url) {
            Ok(formats) => return Ok(formats),
            Err(error) => last_error = error,
        }
    }

    Err(last_error)
}

fn fetch_formats_once(client: &Client, url: &str) -> Result<Vec<SssTwitterFormat>, String> {
    let homepage = client
        .get("https://ssstwitter.com/zh")
        .timeout(RESOLVE_REQUEST_TIMEOUT)
        .headers(default_headers()?)
        .send()
        .map_err(|error| format!("请求 ssstwitter 首页失败：{error}"))?;
    let homepage_status = homepage.status();
    let homepage_html = homepage
        .text()
        .map_err(|error| format!("读取 ssstwitter 首页失败：{error}"))?;

    if !homepage_status.is_success() {
        return Err(format!("ssstwitter 首页返回异常：{homepage_status}"));
    }

    let form_values = extract_form_values(&homepage_html)
        .ok_or_else(|| "无法从 ssstwitter 首页提取表单参数。".to_string())?;

    let endpoint = format!("https://ssstwitter.com{}", form_values.action_path);
    let response = client
        .post(endpoint)
        .timeout(RESOLVE_REQUEST_TIMEOUT)
        .headers(request_headers()?)
        .form(&[
            ("id", url),
            ("locale", "zh"),
            ("tt", form_values.tt.as_str()),
            ("ts", form_values.ts.as_str()),
            ("source", form_values.source.as_str()),
        ])
        .send()
        .map_err(|error| format!("请求 ssstwitter 解析接口失败：{error}"))?;
    let response_status = response.status();
    let response_html = response
        .text()
        .map_err(|error| format!("读取 ssstwitter 解析结果失败：{error}"))?;

    if !response_status.is_success() {
        return Err(format!("ssstwitter 解析接口返回异常：{response_status}"));
    }

    let formats = parse_download_formats(&response_html);
    if formats.is_empty() {
        return Err("ssstwitter 未返回可下载格式。".into());
    }

    Ok(formats)
}

pub fn extract_form_values(html: &str) -> Option<SssTwitterFormValues> {
    let action_path = extract_attr_after(html, r#"data-hx-post=""#)?;
    let include_vals = extract_attr_after(html, r#"include-vals=""#)?;
    let tt = extract_between(&include_vals, "tt:'", "'")?;
    let ts = extract_between(&include_vals, "ts:", ",")?.trim().to_string();
    let source = extract_between(&include_vals, "source:'", "'").unwrap_or_else(|| "form".into());

    Some(SssTwitterFormValues {
        action_path,
        tt,
        ts,
        source,
    })
}

pub fn parse_download_formats(html: &str) -> Vec<SssTwitterFormat> {
    let mut formats = Vec::new();
    let mut start = 0usize;

    while let Some(relative_anchor_start) = html[start..].find("<a") {
        let anchor_start = start + relative_anchor_start;
        let Some(relative_anchor_end) = html[anchor_start..].find("</a>") else {
            break;
        };
        let anchor_end = anchor_start + relative_anchor_end + 4;
        let anchor_html = &html[anchor_start..anchor_end];

        let download_url = extract_attr_after(anchor_html, r#"data-directurl=""#)
            .filter(|value| value.starts_with("https://"))
            .or_else(|| {
                extract_attr_after(anchor_html, r#"href=""#)
                    .filter(|value| value.starts_with("https://"))
            })
            .filter(|value| is_real_download_url(value));

        if let Some(download_url) = download_url {
            let label = extract_between(anchor_html, "<span>", "</span>")
                .map(clean_label)
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "下载".into());
            let ext = if download_url.contains(".mp3") { "mp3" } else { "mp4" }.to_string();

            formats.push(SssTwitterFormat {
                download_url,
                label: label.clone(),
                ext,
                note: format!("来自 ssstwitter 回退：{label}"),
                size_bytes: None,
            });
        }

        start = anchor_end;
    }

    formats
}

/// ssstwitter's result list interleaves the real media download buttons with
/// cross-promotion anchors: app-store ads (play.google.com / apps.apple.com) and
/// bare links to sister sites (https://reelsvideo.io/, https://ssstik.io/, ...).
/// These all share the generic "下载" label, so if the preferred quality label
/// misses they get picked up as `formats.first()`. Keep only genuine media URLs:
/// reject app stores, and reject bare domain roots that carry no real path/query.
fn is_real_download_url(url: &str) -> bool {
    let lowered = url.to_ascii_lowercase();
    const STORE_HOSTS: [&str; 3] = ["play.google.com", "apps.apple.com", "itunes.apple.com"];
    if STORE_HOSTS.iter().any(|host| lowered.contains(host)) {
        return false;
    }

    // Strip scheme, then require a meaningful path segment or a query string.
    // `https://reelsvideo.io/` -> after host the remainder is "/" with no query.
    let after_scheme = match url.split_once("://") {
        Some((_, rest)) => rest,
        None => return false,
    };
    let (host_and_path, query) = match after_scheme.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (after_scheme, None),
    };
    let path = host_and_path.split_once('/').map(|(_, p)| p).unwrap_or("");

    !path.trim_matches('/').is_empty() || query.is_some_and(|q| !q.is_empty())
}

fn clean_label(value: String) -> String {
    value
        .replace("<i class=\"icon icon-ad-rewarded\"></i>", "")
        .replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn extract_attr_after(text: &str, marker: &str) -> Option<String> {
    let start = text.find(marker)? + marker.len();
    let rest = &text[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_between(text: &str, start_marker: &str, end_marker: &str) -> Option<String> {
    let start = text.find(start_marker)? + start_marker.len();
    let rest = &text[start..];
    let end = rest.find(end_marker)?;
    Some(rest[..end].to_string())
}

fn default_headers() -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"));
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"));
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36",
        ),
    );
    Ok(headers)
}

fn request_headers() -> Result<HeaderMap, String> {
    let mut headers = default_headers()?;
    headers.insert("HX-Request", HeaderValue::from_static("true"));
    headers.insert("HX-Target", HeaderValue::from_static("target"));
    headers.insert("HX-Current-URL", HeaderValue::from_static("https://ssstwitter.com/zh"));
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/x-www-form-urlencoded"),
    );
    Ok(headers)
}

fn download_headers() -> Result<HeaderMap, String> {
    let mut headers = default_headers()?;
    // Request the raw media bytes. Some CDN/proxy combinations advertise a content
    // encoding for MP4 responses; letting reqwest transparently decode those bodies
    // can fail mid-stream with `error decoding response body`, even though we only
    // want to persist the original bytes to disk.
    headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));
    headers.insert("referer", HeaderValue::from_static("https://ssstwitter.com/zh"));
    headers.insert("origin", HeaderValue::from_static("https://ssstwitter.com"));
    Ok(headers)
}

#[cfg(test)]
mod tests {
    use super::{
        create_ssstwitter_selection_id, download_headers, download_x_via_ssstwitter_to_path,
        extract_form_values, extract_ssstwitter_selection, open_x_via_ssstwitter_download,
        parse_download_formats,
    };
    use reqwest::header::ACCEPT_ENCODING;
    use std::{fs, io::Read};

    const HOMEPAGE_SNIPPET: &str = r##"
        <form class="hide-after-request"
            data-hx-post="/zh"
            data-hx-target="#target" data-hx-swap="innerHTML" hx-select="#result" hx-push-url="true"
            include-vals="tt:'64d332d5ff528f94919fe8a2db8da9be',ts:1781686812,source:'form'">
    "##;

    const RESULT_SNIPPET: &str = r##"
        <div class="result-container bg-white" id="result_buttons">
            <a data-directurl="https://ssscdn.io/ssstwitter/2066861838516564280/FBsIaI6vL0y70mFX?st=MtaQY_RVTIk3SkkfjRrUXg&e=1781946807"
               class="download_link download-btn quality-best">
                <span>下载 HD 1080x1080<i class="icon icon-ad-rewarded"></i></span>
            </a>
            <a href="https://ssscdn.io/ssstwitter/2066861838516564280/e_bApggw0t5XjvfH?st=HTVnLNAzInkjBYpRFT0yMg&e=1781946807"
               data-directurl=""
               class="download_link download-btn">
                <span>下载 320x320</span>
            </a>
        </div>
    "##;

    #[test]
    fn extracts_ssstwitter_form_values() {
        let values = extract_form_values(HOMEPAGE_SNIPPET).expect("form values should be parsed");

        assert_eq!(values.action_path, "/zh");
        assert_eq!(values.tt, "64d332d5ff528f94919fe8a2db8da9be");
        assert_eq!(values.ts, "1781686812");
        assert_eq!(values.source, "form");
    }

    #[test]
    fn parses_ssstwitter_download_formats() {
        let formats = parse_download_formats(RESULT_SNIPPET);

        assert_eq!(formats.len(), 2);
        assert_eq!(formats[0].label, "下载 HD 1080x1080");
        assert!(formats[0].download_url.contains("ssscdn.io/ssstwitter/2066861838516564280"));
        assert_eq!(formats[1].label, "下载 320x320");
        assert_eq!(formats[1].ext, "mp4");
    }

    #[test]
    fn download_headers_request_identity_encoding() {
        let headers = download_headers().expect("download headers should build");

        assert_eq!(
            headers.get(ACCEPT_ENCODING).and_then(|value| value.to_str().ok()),
            Some("identity")
        );
    }

    #[test]
    fn round_trips_ssstwitter_selection_id_with_direct_url() {
        let url = "https://ssscdn.io/ssstwitter/2066861838516564280/abc?st=x&e=1";
        let selection_id = create_ssstwitter_selection_id("下载 HD 1080x1080", Some(url));
        let selection = extract_ssstwitter_selection(&selection_id).expect("should decode");

        assert_eq!(selection.label, "下载 HD 1080x1080");
        assert_eq!(selection.direct_url.as_deref(), Some(url));
    }

    #[test]
    fn round_trips_ssstwitter_selection_id_without_direct_url() {
        let selection_id = create_ssstwitter_selection_id("下载 320x320", None);
        let selection = extract_ssstwitter_selection(&selection_id).expect("should decode");

        assert_eq!(selection.label, "下载 320x320");
        assert_eq!(selection.direct_url, None);
    }

    const RESULT_WITH_ADS_SNIPPET: &str = r##"
        <div class="result-container" id="result_buttons">
            <a href="https://play.google.com/store/apps/details?id=com.fget.facebook.video.downloader.twitter.saver&utm_source=ssstw&utm_medium=header"
               class="ad-header"><span>下载</span></a>
            <a data-directurl="https://ssscdn.io/ssstwitter/2066861838516564280/FBsIaI6vL0y70mFX?st=abc&e=1781955733"
               class="download_link"><span>下载 HD 1080x1080</span></a>
            <a href="https://ssscdn.io/ssstwitter/2066861838516564280/e_bApggw0t5XjvfH?st=def&e=1781955733"
               class="download_link"><span>下载 320x320</span></a>
            <a href="https://reelsvideo.io/" class="cross-promo"><span>下载</span></a>
            <a href="https://ssstik.io/" class="cross-promo"><span>下载</span></a>
        </div>
    "##;

    #[test]
    fn filters_out_ad_and_cross_promo_anchors() {
        let formats = parse_download_formats(RESULT_WITH_ADS_SNIPPET);

        assert_eq!(formats.len(), 2, "only real media links should remain");
        assert_eq!(formats[0].label, "下载 HD 1080x1080");
        assert_eq!(formats[1].label, "下载 320x320");
        assert!(formats.iter().all(|f| f.download_url.contains("ssscdn.io")));
    }

    #[test]
    #[ignore = "requires live ssstwitter network access"]
    fn opens_live_ssstwitter_download_for_target_x_url() {
        let (mut response, selected) = open_x_via_ssstwitter_download(
            "https://x.com/Caughtgirls1/status/2066861838516564280/video/1",
            Some("下载 HD 1080x1080"),
        )
        .expect("live ssstwitter download should open");

        assert_eq!(selected.ext, "mp4");
        assert!(response.content_length().unwrap_or(0) > 0);

        let mut prefix = [0u8; 16];
        let bytes_read = response.read(&mut prefix).expect("response should be readable");
        assert!(bytes_read > 0);
    }

    #[test]
    #[ignore = "requires live ssstwitter network access"]
    fn downloads_live_ssstwitter_video_to_temp_file() {
        let temp_dir = std::env::temp_dir().join("swell-video-downloader-tests");
        fs::create_dir_all(&temp_dir).expect("temp directory should be created");
        let output_path = temp_dir.join("x-live-download.mp4");

        let result = download_x_via_ssstwitter_to_path(
            "https://x.com/Caughtgirls1/status/2066861838516564280/video/1",
            Some("下载 320x320"),
            &output_path,
            Some(2 * 1024 * 1024),
            |_downloaded, _total| {},
        )
        .expect("live ssstwitter video should download");

        assert_eq!(result.selected_format.ext, "mp4");
        assert!(result.downloaded_bytes >= 2 * 1024 * 1024);
        assert!(output_path.exists());
        assert!(fs::metadata(&output_path).expect("output file metadata should exist").len() > 0);

        let _ = fs::remove_file(output_path);
    }

    #[test]
    #[ignore = "requires live ssstwitter network access"]
    fn downloads_requested_x_video_to_temp_file() {
        let temp_dir = std::env::temp_dir().join("swell-video-downloader-tests");
        fs::create_dir_all(&temp_dir).expect("temp directory should be created");
        let output_path = temp_dir.join("x-target-download.mp4");

        let result = download_x_via_ssstwitter_to_path(
            "https://x.com/4Brazzerlive/status/2068239068916507115/video/1",
            Some("下载 HD 1080x1080"),
            &output_path,
            Some(2 * 1024 * 1024),
            |_downloaded, _total| {},
        )
        .expect("live ssstwitter video should download");

        assert_eq!(result.selected_format.ext, "mp4");
        assert!(fs::metadata(&output_path).expect("output file metadata should exist").len() > 0);

        let _ = fs::remove_file(output_path);
    }
}
