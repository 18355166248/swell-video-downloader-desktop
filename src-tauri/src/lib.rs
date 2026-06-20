mod commands;
mod downloader;
mod events;
mod platform;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::resolve::resolve_media,
            commands::download::start_download,
            commands::cookies::list_cookie_sources,
            commands::system::check_dependencies
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
