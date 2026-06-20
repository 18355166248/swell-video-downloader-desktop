use reqwest::blocking::{Client, Response};
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};

pub const SSSTWITTER_SELECTION_PREFIX: &str = "ssstwitter:";

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SssTwitterDownloadResult {
    pub selected_format: SssTwitterFormat,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
}

pub fn resolve_x_via_ssstwitter(url: &str) -> Result<Vec<SssTwitterFormat>, String> {
    let client = create_client()?;
    fetch_formats_with_client(&client, url)
}

pub fn create_ssstwitter_selection_id(label: &str) -> String {
    format!("{SSSTWITTER_SELECTION_PREFIX}{}", label.trim())
}

pub fn extract_ssstwitter_selection_label(selection_id: &str) -> Option<String> {
    selection_id
        .strip_prefix(SSSTWITTER_SELECTION_PREFIX)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn open_x_via_ssstwitter_download(
    url: &str,
    preferred_label: Option<&str>,
) -> Result<(Response, SssTwitterFormat), String> {
    let client = create_client()?;
    let formats = fetch_formats_with_client(&client, url)?;

    let preferred_label = preferred_label.map(str::trim).filter(|value| !value.is_empty());
    let selected = preferred_label
        .and_then(|label| formats.iter().find(|format| format.label == label).cloned())
        .or_else(|| formats.first().cloned())
        .ok_or_else(|| "ssstwitter 未返回可下载格式。".to_string())?;

    let response = client
        .get(selected.download_url.as_str())
        .headers(download_headers()?)
        .send()
        .map_err(|error| format!("请求 ssstwitter 下载地址失败：{error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "ssstwitter 下载地址返回异常：{}",
            response.status()
        ));
    }

    if response.content_length() == Some(0) {
        return Err("ssstwitter 返回了空文件响应，当前下载链接可能已失效。".into());
    }

    Ok((response, selected))
}

pub fn download_x_via_ssstwitter_to_path<F>(
    url: &str,
    preferred_label: Option<&str>,
    output_path: &Path,
    max_bytes: Option<u64>,
    mut on_progress: F,
) -> Result<SssTwitterDownloadResult, String>
where
    F: FnMut(u64, Option<u64>),
{
    let (mut response, selected_format) = open_x_via_ssstwitter_download(url, preferred_label)?;
    let total_bytes = response.content_length();
    let mut downloaded_bytes = 0u64;
    let mut file = File::create(output_path).map_err(|error| format!("创建输出文件失败：{error}"))?;
    let mut buffer = [0u8; 64 * 1024];

    loop {
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

        if let Some(limit) = max_bytes {
            if downloaded_bytes >= limit {
                break;
            }
        }
    }

    Ok(SssTwitterDownloadResult {
        selected_format,
        downloaded_bytes,
        total_bytes,
    })
}

fn create_client() -> Result<Client, String> {
    Client::builder()
        .cookie_store(true)
        .build()
        .map_err(|error| format!("创建 ssstwitter 请求客户端失败：{error}"))
}

fn fetch_formats_with_client(client: &Client, url: &str) -> Result<Vec<SssTwitterFormat>, String> {
    let homepage = client
        .get("https://ssstwitter.com/zh")
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
            });

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
            });
        }

        start = anchor_end;
    }

    formats
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
    headers.insert("referer", HeaderValue::from_static("https://ssstwitter.com/zh"));
    headers.insert("origin", HeaderValue::from_static("https://ssstwitter.com"));
    Ok(headers)
}

#[cfg(test)]
mod tests {
    use super::{
        create_ssstwitter_selection_id, download_x_via_ssstwitter_to_path, extract_form_values,
        extract_ssstwitter_selection_label, open_x_via_ssstwitter_download, parse_download_formats,
    };
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
    fn round_trips_ssstwitter_selection_id() {
        let selection_id = create_ssstwitter_selection_id("下载 HD 1080x1080");

        assert_eq!(
            extract_ssstwitter_selection_label(&selection_id).as_deref(),
            Some("下载 HD 1080x1080")
        );
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
}
