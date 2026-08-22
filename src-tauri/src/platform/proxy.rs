//! Proxy detection.
//!
//! `reqwest` honors the standard proxy environment variables, but it does NOT read
//! the Windows system proxy (the registry setting configured by Clash/V2Ray/etc.).
//! A Tauri app launched without those env vars therefore connects directly — which,
//! for an overseas CDN like ssstwitter/Twitter behind the GFW, crawls at ~15 KiB/s
//! while the local proxy delivers >1 MiB/s. We bridge that gap here.

#[cfg(windows)]
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[cfg(windows)]
use crate::platform::spawn::hide_console_window;

/// How long a detection result stays good. On Windows the lookup shells out to
/// `reg query` twice, and every resolve, download, page-title fetch and ssstwitter
/// request asks for the proxy — a 50-link batch would otherwise spawn hundreds of
/// processes. Short enough that toggling the proxy still takes effect on its own.
const PROXY_CACHE_TTL: Duration = Duration::from_secs(30);

static PROXY_CACHE: OnceLock<Mutex<Option<(Instant, Option<String>)>>> = OnceLock::new();

fn proxy_cache() -> &'static Mutex<Option<(Instant, Option<String>)>> {
    PROXY_CACHE.get_or_init(|| Mutex::new(None))
}

/// Returns a proxy URL (e.g. `http://127.0.0.1:7897`) to route requests through,
/// preferring the standard env vars and falling back to the Windows system proxy.
/// The answer is cached for [`PROXY_CACHE_TTL`].
pub fn detect_proxy() -> Option<String> {
    if let Ok(cache) = proxy_cache().lock() {
        if let Some((checked_at, proxy)) = cache.as_ref() {
            if checked_at.elapsed() < PROXY_CACHE_TTL {
                return proxy.clone();
            }
        }
    }

    let detected = detect_proxy_uncached();

    if let Ok(mut cache) = proxy_cache().lock() {
        *cache = Some((Instant::now(), detected.clone()));
    }

    detected
}

/// Drop the cached result so the next [`detect_proxy`] call re-detects. Used by the
/// tests; the app itself relies on the TTL.
#[cfg(test)]
fn clear_proxy_cache() {
    if let Ok(mut cache) = proxy_cache().lock() {
        *cache = None;
    }
}

fn detect_proxy_uncached() -> Option<String> {
    if let Some(proxy) = env_proxy() {
        return Some(proxy);
    }

    #[cfg(windows)]
    {
        return windows_system_proxy();
    }

    #[cfg(not(windows))]
    {
        None
    }
}

fn env_proxy() -> Option<String> {
    const VARS: [&str; 6] = [
        "HTTPS_PROXY",
        "https_proxy",
        "ALL_PROXY",
        "all_proxy",
        "HTTP_PROXY",
        "http_proxy",
    ];
    for var in VARS {
        if let Ok(value) = std::env::var(var) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(ensure_scheme(value));
            }
        }
    }
    None
}

#[cfg(windows)]
fn windows_system_proxy() -> Option<String> {
    const KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings";

    // Only honor it when proxying is actually enabled (ProxyEnable == 0x1).
    let enabled = reg_query(KEY, "ProxyEnable")?;
    if !enabled.trim().ends_with("0x1") {
        return None;
    }

    parse_proxy_server(reg_query(KEY, "ProxyServer")?.trim())
}

#[cfg(windows)]
fn reg_query(key: &str, value: &str) -> Option<String> {
    let mut cmd = Command::new("reg");
    hide_console_window(&mut cmd);
    let output = cmd
        .args(["query", key, "/v", value])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    // Lines look like: "    ProxyServer    REG_SZ    127.0.0.1:7897"
    for line in text.lines() {
        if let Some(idx) = line.find(value) {
            let parts: Vec<&str> = line[idx + value.len()..].split_whitespace().collect();
            // [REG_SZ | REG_DWORD, <value...>]
            if parts.len() >= 2 {
                return Some(parts[1..].join(" "));
            }
        }
    }
    None
}

/// `ProxyServer` is either a bare `host:port` (applies to all protocols) or a
/// per-protocol list like `http=host:port;https=host:port;socks=host:port`.
fn parse_proxy_server(server: &str) -> Option<String> {
    if server.is_empty() {
        return None;
    }

    if server.contains('=') {
        let segments: Vec<&str> = server.split(';').collect();
        for prefix in ["https=", "http=", "socks="] {
            if let Some(found) = segments.iter().find_map(|s| s.trim().strip_prefix(prefix)) {
                return Some(ensure_scheme(found.trim()));
            }
        }
        // Unknown layout: take the first segment's value.
        let value = segments.first()?.split('=').nth(1)?;
        return Some(ensure_scheme(value.trim()));
    }

    Some(ensure_scheme(server))
}

fn ensure_scheme(value: &str) -> String {
    const SCHEMES: [&str; 4] = ["http://", "https://", "socks5://", "socks5h://"];
    if SCHEMES.iter().any(|scheme| value.starts_with(scheme)) {
        value.to_string()
    } else {
        format!("http://{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::{clear_proxy_cache, detect_proxy, ensure_scheme, parse_proxy_server};

    #[test]
    fn parses_bare_host_port() {
        assert_eq!(
            parse_proxy_server("127.0.0.1:7897").as_deref(),
            Some("http://127.0.0.1:7897")
        );
    }

    #[test]
    fn parses_per_protocol_list_preferring_https() {
        assert_eq!(
            parse_proxy_server("http=127.0.0.1:1;https=127.0.0.1:7897").as_deref(),
            Some("http://127.0.0.1:7897")
        );
    }

    #[test]
    fn keeps_explicit_scheme() {
        assert_eq!(ensure_scheme("socks5://127.0.0.1:7897"), "socks5://127.0.0.1:7897");
    }

    /// The cache is what keeps a 50-link batch from shelling out to `reg query`
    /// hundreds of times: a change made after the first lookup is not observed
    /// again until the TTL expires.
    #[test]
    fn repeated_detection_reuses_the_cached_result() {
        clear_proxy_cache();
        std::env::set_var("HTTPS_PROXY", "127.0.0.1:7897");
        let first = detect_proxy();
        assert_eq!(first.as_deref(), Some("http://127.0.0.1:7897"));

        std::env::set_var("HTTPS_PROXY", "127.0.0.1:1080");
        assert_eq!(detect_proxy(), first, "the cached value should be reused");

        clear_proxy_cache();
        assert_eq!(detect_proxy().as_deref(), Some("http://127.0.0.1:1080"));

        std::env::remove_var("HTTPS_PROXY");
        clear_proxy_cache();
    }
}
