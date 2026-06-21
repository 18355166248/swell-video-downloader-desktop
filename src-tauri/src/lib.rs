mod commands;
mod downloader;
mod events;
mod platform;

use std::fs::{self, OpenOptions};
use std::io;
use std::path::PathBuf;

use log::LevelFilter;
use tauri::Manager;

/// Resolve the log file path: <app_data>/logs/swell.log
fn resolve_log_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    let data_dir = app.path().app_data_dir().ok()?;
    let log_dir = data_dir.join("logs");
    fs::create_dir_all(&log_dir).ok()?;
    Some(log_dir.join("swell.log"))
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
