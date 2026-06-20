use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, USER_AGENT};
use std::time::Duration;

const PAGE_TITLE_TIMEOUT: Duration = Duration::from_secs(12);

pub fn fetch_page_title(url: &str) -> Option<String> {
    let client = create_client().ok()?;
    let response = client
        .get(url)
        .headers(default_headers().ok()?)
        .timeout(PAGE_TITLE_TIMEOUT)
        .send()
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let html = response.text().ok()?;
    parse_page_title(&html)
}

pub fn parse_page_title(html: &str) -> Option<String> {
    extract_meta_content(html, "property", "og:title")
        .or_else(|| extract_meta_content(html, "name", "twitter:title"))
        .or_else(|| extract_tag_inner_text(html, "title"))
        .and_then(|value| normalize_title(&value))
}

fn create_client() -> Result<Client, String> {
    let mut builder = Client::builder().connect_timeout(PAGE_TITLE_TIMEOUT);
    if let Some(proxy_url) = crate::platform::proxy::detect_proxy() {
        if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
            builder = builder.proxy(proxy);
        }
    }
    builder
        .build()
        .map_err(|error| format!("创建页面标题请求客户端失败：{error}"))
}

fn default_headers() -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"),
    );
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"));
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36",
        ),
    );
    Ok(headers)
}

fn extract_meta_content(html: &str, key_name: &str, key_value: &str) -> Option<String> {
    let lower_html = html.to_ascii_lowercase();
    let lower_key = format!(r#"{key_name}="{key_value}""#).to_ascii_lowercase();
    let lower_key_single = format!(r#"{key_name}='{key_value}'"#).to_ascii_lowercase();
    let lower_content = r#"content=""#;
    let lower_content_single = "content='";

    let mut search_start = 0usize;
    while let Some(meta_start_rel) = lower_html[search_start..].find("<meta") {
        let meta_start = search_start + meta_start_rel;
        let meta_end = lower_html[meta_start..]
            .find('>')
            .map(|offset| meta_start + offset)
            .unwrap_or(lower_html.len());
        let meta_tag = &html[meta_start..meta_end];
        let meta_tag_lower = &lower_html[meta_start..meta_end];

        let has_target =
            meta_tag_lower.contains(&lower_key) || meta_tag_lower.contains(&lower_key_single);
        if has_target {
            if let Some(idx) = meta_tag_lower.find(lower_content) {
                let value_start = idx + lower_content.len();
                if let Some(value_end) = meta_tag[value_start..].find('"') {
                    return Some(meta_tag[value_start..value_start + value_end].to_string());
                }
            }
            if let Some(idx) = meta_tag_lower.find(lower_content_single) {
                let value_start = idx + lower_content_single.len();
                if let Some(value_end) = meta_tag[value_start..].find('\'') {
                    return Some(meta_tag[value_start..value_start + value_end].to_string());
                }
            }
        }

        search_start = meta_end.saturating_add(1);
    }

    None
}

fn extract_tag_inner_text(html: &str, tag: &str) -> Option<String> {
    let lower_html = html.to_ascii_lowercase();
    let open_tag = format!("<{tag}");
    let close_tag = format!("</{tag}>");
    let start = lower_html.find(&open_tag)?;
    let content_start = lower_html[start..].find('>').map(|offset| start + offset + 1)?;
    let end = lower_html[content_start..]
        .find(&close_tag)
        .map(|offset| content_start + offset)?;
    Some(html[content_start..end].to_string())
}

fn normalize_title(raw: &str) -> Option<String> {
    let mut value = raw
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">");
    value = value.replace('\n', " ");
    let mut title = value.split_whitespace().collect::<Vec<_>>().join(" ");

    for suffix in [
        " / X",
        " / Twitter",
        " / X.com",
        " - Pornhub.com",
        " - Pornhub",
        " | Pornhub",
    ] {
        if title.ends_with(suffix) {
            title.truncate(title.len().saturating_sub(suffix.len()));
            title = title.trim().to_string();
        }
    }

    let title = title.trim_matches(|ch: char| ch == '-' || ch == '|' || ch.is_whitespace());
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::parse_page_title;

    #[test]
    fn prefers_og_title_and_strips_x_suffix() {
        let html = r#"
            <html><head>
              <meta property="og:title" content="A cool clip / X" />
              <title>fallback</title>
            </head></html>
        "#;

        assert_eq!(parse_page_title(html).as_deref(), Some("A cool clip"));
    }

    #[test]
    fn falls_back_to_twitter_title_then_title_tag() {
        let html = r#"
            <html><head>
              <meta name="twitter:title" content="A story - Pornhub.com" />
            </head></html>
        "#;
        assert_eq!(parse_page_title(html).as_deref(), Some("A story"));

        let html = "<html><head><title>Visible page title</title></head></html>";
        assert_eq!(parse_page_title(html).as_deref(), Some("Visible page title"));
    }
}
