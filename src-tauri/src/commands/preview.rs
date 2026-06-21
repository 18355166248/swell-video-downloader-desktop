use std::{
    fs,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
use tauri::AppHandle;

use crate::{
    downloader::x_ssstwitter::{download_selection_to_path, extract_ssstwitter_selection},
    platform::binaries::resolve_ffmpeg,
    platform::spawn::hide_console_window,
};

/// ssstwitter never returns a poster image, so we synthesize a preview frame:
/// stream a small prefix of the chosen clip and let ffmpeg decode its first frame
/// into a PNG, returned to the UI as a `data:` URL. Capped small so it is fast and
/// works even for the huge HD variants (Twitter MP4s are faststart, so the first
/// frame sits near the front of the file).
const PREVIEW_PREFIX_BYTES: u64 = 3 * 1024 * 1024;

static PREVIEW_COUNTER: AtomicU64 = AtomicU64::new(1);

// Blocking work (reqwest::blocking download + ffmpeg) must run on a blocking thread
// via `spawn_blocking` — see the note on `resolve_media` for why the async runtime
// and the main thread are both wrong here.
#[tauri::command]
pub async fn generate_preview(
    app: AppHandle,
    url: String,
    format_id: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || generate_preview_blocking(app, url, format_id))
        .await
        .map_err(|error| format!("预览任务执行失败：{error}"))?
}

fn generate_preview_blocking(app: AppHandle, url: String, format_id: String) -> Result<String, String> {
    let selection = extract_ssstwitter_selection(&format_id)
        .ok_or_else(|| "当前来源不支持生成预览。".to_string())?;

    let ffmpeg = resolve_ffmpeg(&app).ok_or_else(|| {
        "未找到 ffmpeg，无法生成预览。请将其放到 resources/bin 目录，或通过 SWELL_FFMPEG_PATH 指定路径。"
            .to_string()
    })?;

    let work_dir = std::env::temp_dir().join("swell-video-downloader-preview");
    fs::create_dir_all(&work_dir).map_err(|error| format!("创建预览临时目录失败：{error}"))?;

    let unique = PREVIEW_COUNTER.fetch_add(1, Ordering::Relaxed);
    let clip_path = work_dir.join(format!("preview-{unique}.mp4"));
    let frame_path = work_dir.join(format!("preview-{unique}.png"));

    download_selection_to_path(
        &url,
        selection.direct_url.as_deref(),
        Some(selection.label.as_str()),
        &clip_path,
        Some(PREVIEW_PREFIX_BYTES),
        || false,
        |_downloaded, _total| {},
    )?;

    let mut ffmpeg_cmd = Command::new(&ffmpeg.path);
    hide_console_window(&mut ffmpeg_cmd);
    let output = ffmpeg_cmd
        .arg("-y")
        .arg("-i")
        .arg(&clip_path)
        .arg("-frames:v")
        .arg("1")
        .arg("-vf")
        .arg("scale=480:-2")
        .arg("-f")
        .arg("image2")
        .arg(&frame_path)
        .output()
        .map_err(|error| format!("启动 ffmpeg 生成预览失败：{error}"));

    let _ = fs::remove_file(&clip_path);

    let output = output?;
    if !output.status.success() {
        let _ = fs::remove_file(&frame_path);
        return Err(format!(
            "ffmpeg 生成预览失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let png_bytes = fs::read(&frame_path).map_err(|error| format!("读取预览图失败：{error}"))?;
    let _ = fs::remove_file(&frame_path);

    Ok(format!("data:image/png;base64,{}", base64_encode(&png_bytes)))
}

fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(input.len().div_ceil(3) * 4);

    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        output.push(ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
        output.push(ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
        output.push(if chunk.len() > 1 {
            ALPHABET[((triple >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            ALPHABET[(triple & 0x3f) as usize] as char
        } else {
            '='
        });
    }

    output
}

#[cfg(test)]
mod tests {
    use super::base64_encode;

    #[test]
    fn encodes_base64_like_reference_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }
}
