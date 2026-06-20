use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Clone)]
pub struct BinaryResolution {
    pub path: PathBuf,
    pub source: &'static str,
}

pub fn resolve_yt_dlp(app: &AppHandle) -> Option<BinaryResolution> {
    resolve_binary(app, "yt-dlp")
}

pub fn resolve_ffmpeg(app: &AppHandle) -> Option<BinaryResolution> {
    resolve_binary(app, "ffmpeg")
}

fn resolve_binary(app: &AppHandle, base_name: &str) -> Option<BinaryResolution> {
    candidate_paths(app, base_name)
        .into_iter()
        .find(|candidate| candidate.path.is_file())
}

fn candidate_paths(app: &AppHandle, base_name: &str) -> Vec<BinaryResolution> {
    let file_name = executable_name(base_name);
    let platform_dir = platform_dir();
    let mut candidates = Vec::new();

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(BinaryResolution {
            path: resource_dir.join("resources").join("bin").join(platform_dir).join(&file_name),
            source: "bundled",
        });
        candidates.push(BinaryResolution {
            path: resource_dir.join("bin").join(platform_dir).join(&file_name),
            source: "bundled",
        });
    }

    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    candidates.push(BinaryResolution {
        path: project_root.join("resources").join("bin").join(platform_dir).join(&file_name),
        source: "project",
    });

    if let Some(path_from_env) = path_from_env(base_name) {
        candidates.push(BinaryResolution {
            path: path_from_env,
            source: "env",
        });
    }

    candidates.push(BinaryResolution {
        path: PathBuf::from(&file_name),
        source: "system",
    });

    candidates
}

fn path_from_env(base_name: &str) -> Option<PathBuf> {
    let key = match base_name {
        "yt-dlp" => "SWELL_YTDLP_PATH",
        "ffmpeg" => "SWELL_FFMPEG_PATH",
        _ => return None,
    };

    std::env::var_os(key).map(PathBuf::from)
}

fn executable_name(base_name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{base_name}.exe")
    } else {
        base_name.to_string()
    }
}

fn platform_dir() -> &'static str {
    if cfg!(target_os = "windows") {
        "win"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    }
}
