use serde::Serialize;

#[derive(Serialize)]
pub struct CookieSource {
    pub id: String,
    pub label: String,
}

#[tauri::command]
pub fn list_cookie_sources() -> Vec<CookieSource> {
    vec![
        CookieSource {
            id: "chrome".into(),
            label: "Chrome".into(),
        },
        CookieSource {
            id: "edge".into(),
            label: "Edge".into(),
        },
        CookieSource {
            id: "import".into(),
            label: "手动导入".into(),
        },
    ]
}
