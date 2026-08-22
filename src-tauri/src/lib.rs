mod commands;
mod downloader;
mod events;
mod platform;

use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use log::LevelFilter;
use tauri::Manager;

/// Resolve the log file path: <app_data>/logs/swell.log
fn resolve_log_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    let data_dir = app.path().app_data_dir().ok()?;
    let log_dir = data_dir.join("logs");
    fs::create_dir_all(&log_dir).ok()?;
    Some(log_dir.join("swell.log"))
}

/// Roll the log over once it passes this size. Every run appends to the same file
/// and downloads log heavily (a failed task dumps yt-dlp's output), so without a
/// cap `swell.log` grows for as long as the app is installed.
const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

/// Move an oversized log to `<name>.1`, replacing the previous generation, so one
/// backup survives while the live file starts empty. Checked once per launch:
/// env_logger has no rotation of its own, and re-checking on every write would put
/// a `metadata` syscall in front of each log line.
fn rotate_log_if_needed(path: &Path, max_bytes: u64) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.len() < max_bytes {
        return;
    }

    let backup = path.with_extension("log.1");
    let _ = fs::remove_file(&backup);
    let _ = fs::rename(path, &backup);
}

/// A writer that tees output to both stderr and a log file.
struct TeeWriter {
    file: fs::File,
}

impl io::Write for TeeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Write to stderr (visible in dev console)
        let _ = io::stderr().write(buf);
        // Write to file
        self.file.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let _ = io::stderr().flush();
        self.file.flush()
    }
}

fn init_logging(app: &tauri::AppHandle) {
    let log_path = resolve_log_path(app);

    if let Some(ref path) = log_path {
        rotate_log_if_needed(path, MAX_LOG_BYTES);
        if let Ok(file) = OpenOptions::new().create(true).append(true).open(path) {
            let tee = TeeWriter { file };
            env_logger::Builder::new()
                .filter_level(LevelFilter::Info)
                .format_timestamp_millis()
                .target(env_logger::Target::Pipe(Box::new(tee)))
                .init();
            log::info!("=== Swell Video Downloader started ===");
            log::info!("log file: {}", path.display());
            return;
        }
    }

    // Fallback: stderr only (dev mode)
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info)
        .format_timestamp_millis()
        .init();
    log::info!("=== Swell Video Downloader started (stderr only) ===");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            init_logging(&app.handle());
            // Clear `.part` files a previous run never got to clean up.
            commands::download::sweep_stale_staging_files(&app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::resolve::resolve_media,
            commands::resolve::diagnose_media,
            commands::instagram::collect_instagram_targets,
            commands::download::start_download,
            commands::download::cancel_download,
            commands::download::get_download_dir,
            commands::download::get_download_dir_settings,
            commands::download::set_download_dir,
            commands::download::reset_download_dir,
            commands::download::get_app_settings,
            commands::download::set_app_settings,
            commands::preview::generate_preview,
            commands::cookies::list_cookie_sources,
            commands::system::check_dependencies
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{rotate_log_if_needed, MAX_LOG_BYTES};
    use std::{env, fs};

    #[test]
    fn oversized_log_is_moved_aside_and_keeps_one_backup() {
        let dir = env::temp_dir().join(format!("swell-log-rotate-test-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("fixture directory should be writable");
        let log_path = dir.join("swell.log");
        let backup_path = dir.join("swell.log.1");

        fs::write(&backup_path, "older run").expect("backup fixture should be writable");
        fs::write(&log_path, "0123456789").expect("log fixture should be writable");

        rotate_log_if_needed(&log_path, 4);

        assert!(!log_path.exists(), "the oversized log should be moved aside");
        assert_eq!(
            fs::read_to_string(&backup_path).expect("backup should exist"),
            "0123456789",
            "the previous backup should be replaced by the rotated log"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn log_under_the_limit_is_left_alone() {
        let dir = env::temp_dir().join(format!("swell-log-keep-test-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("fixture directory should be writable");
        let log_path = dir.join("swell.log");
        fs::write(&log_path, "still small").expect("log fixture should be writable");

        rotate_log_if_needed(&log_path, MAX_LOG_BYTES);

        assert!(log_path.exists(), "a small log should stay in place");
        assert!(!dir.join("swell.log.1").exists(), "no backup should be created");

        let _ = fs::remove_dir_all(&dir);
    }
}
