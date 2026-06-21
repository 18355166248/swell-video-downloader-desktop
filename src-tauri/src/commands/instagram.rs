use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::commands::instagram_types::{
    CollectInstagramTargetsRequest, CollectInstagramTargetsResponse,
};

pub fn validate_collect_request(
    request: &CollectInstagramTargetsRequest,
) -> Result<(), String> {
    if !request.url.contains("instagram.com") {
        return Err("仅支持 Instagram 链接".into());
    }

    if request.count == 0 {
        return Err("抓取数量必须大于 0".into());
    }

    Ok(())
}

fn parse_collector_output(raw: &str) -> Result<CollectInstagramTargetsResponse, String> {
    serde_json::from_str(raw).map_err(|error| format!("解析 Instagram 采集结果失败：{error}"))
}

/// Locate `scripts/instagram-collector.mjs`. During `tauri dev` the binary's cwd
/// is the `src-tauri` directory, while the script lives at the repo root, so we
/// probe a few candidate locations before falling back to the plain relative path.
fn resolve_collector_script() -> PathBuf {
    let candidates = [
        PathBuf::from("scripts/instagram-collector.mjs"),
        PathBuf::from("../scripts/instagram-collector.mjs"),
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../scripts/instagram-collector.mjs"),
    ];

    candidates
        .iter()
        .find(|path| path.is_file())
        .cloned()
        .unwrap_or_else(|| PathBuf::from("scripts/instagram-collector.mjs"))
}

fn run_collector(
    request: &CollectInstagramTargetsRequest,
) -> Result<CollectInstagramTargetsResponse, String> {
    let script = resolve_collector_script();
    let mut command = Command::new("node");
    command
        .arg(&script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Instagram is reached through the detected system proxy; the collector reads
    // it from the standard proxy env vars (same rationale as the yt-dlp path).
    if let Some(proxy) = crate::platform::proxy::detect_proxy() {
        command.env("HTTPS_PROXY", &proxy);
        command.env("HTTP_PROXY", &proxy);
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("启动 Instagram 采集脚本失败：{error}"))?;

    let payload =
        serde_json::to_vec(request).map_err(|error| format!("序列化采集参数失败：{error}"))?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(&payload)
            .map_err(|error| format!("写入采集参数失败：{error}"))?;
    }
    // Drop stdin so the collector's stdin reader sees EOF and proceeds.
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .map_err(|error| format!("等待 Instagram 采集脚本结束失败：{error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "Instagram 采集脚本执行失败".into()
        } else {
            stderr
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    parse_collector_output(&stdout)
}

#[tauri::command]
pub async fn collect_instagram_targets(
    request: CollectInstagramTargetsRequest,
) -> Result<CollectInstagramTargetsResponse, String> {
    validate_collect_request(&request)?;
    tauri::async_runtime::spawn_blocking(move || run_collector(&request))
        .await
        .map_err(|error| format!("Instagram 采集任务执行失败：{error}"))?
}

#[cfg(test)]
mod tests {
    use super::{parse_collector_output, validate_collect_request};
    use crate::commands::instagram_types::{
        CollectInstagramTargetsRequest, InstagramCollectMode,
    };

    #[test]
    fn rejects_non_instagram_url() {
        let request = CollectInstagramTargetsRequest {
            url: "https://example.com/demo".into(),
            mode: InstagramCollectMode::Single,
            count: 1,
            sessionid: None,
            cookie_file_path: None,
        };

        let error = validate_collect_request(&request).expect_err("should reject host");
        assert!(error.contains("Instagram"));
    }

    #[test]
    fn rejects_zero_count() {
        let request = CollectInstagramTargetsRequest {
            url: "https://www.instagram.com/p/abc123/".into(),
            mode: InstagramCollectMode::Single,
            count: 0,
            sessionid: None,
            cookie_file_path: None,
        };

        let error = validate_collect_request(&request).expect_err("should reject zero count");
        assert!(error.contains("数量"));
    }

    #[test]
    fn parses_collector_stdout_json() {
        let raw = r#"{"items":[{"url":"https://www.instagram.com/p/abc/","kind":"post","source_label":"single","thumbnail_hint":null}],"resolved_count":1,"warnings":[],"cookie_bridge_file_path":null}"#;
        let parsed = parse_collector_output(raw).expect("should parse");
        assert_eq!(parsed.resolved_count, 1);
        assert_eq!(parsed.items[0].kind, "post");
    }
}
