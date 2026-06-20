use serde::Serialize;

#[derive(Serialize)]
pub struct CookieSource {
    pub id: String,
    pub label: String,
}

#[tauri::command]
pub fn list_cookie_sources() -> Vec<CookieSource> {
    vec![
        // Default first. x.com resolves via ssstwitter (no cookies needed); reading
        // browser cookies up front is what stalls resolve when Chrome/Edge is open.
        CookieSource {
            id: "none".into(),
            label: "无 Cookie（推荐 x.com）".into(),
        },
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
