//! Which site a URL belongs to.
//!
//! Matching is done on the parsed host, never on the raw URL string. A substring
//! test looks equivalent and is not: `netflix.com` and `vox.com` both contain
//! "x.com", and any URL can carry another site's name in a path or query
//! parameter. Mirrors `SUPPORTED_VIDEO_SITES` in `src/lib/supported-sites.ts` —
//! keep the two lists in sync.

/// Checked in order, so a host that is a subdomain of several sites resolves to
/// the first listed.
const SUPPORTED_SITES: [&str; 3] = ["instagram.com", "x.com", "pornhub.com"];

/// The site host a URL belongs to, or `None` when it is not a supported site.
pub fn detect_site(url: &str) -> Option<&'static str> {
    let host = url_host(url)?;
    SUPPORTED_SITES
        .into_iter()
        .find(|site| host_matches(&host, site))
}

/// Whether a URL points at `site` — that exact host, or a subdomain of it.
pub fn is_site(url: &str, site: &str) -> bool {
    url_host(url)
        .map(|host| host_matches(&host, site))
        .unwrap_or(false)
}

fn host_matches(host: &str, site: &str) -> bool {
    host == site || host.ends_with(&format!(".{site}"))
}

/// Lowercased host without a `www.` prefix. Scheme-less input (`x.com/i/status/1`)
/// is retried as https, since that is how a user pastes a link.
fn url_host(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parsed = reqwest::Url::parse(trimmed)
        .or_else(|_| reqwest::Url::parse(&format!("https://{trimmed}")))
        .ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();

    Some(host.strip_prefix("www.").unwrap_or(&host).to_string())
}

#[cfg(test)]
mod tests {
    use super::{detect_site, is_site};

    #[test]
    fn detects_the_supported_sites() {
        assert_eq!(detect_site("https://x.com/user/status/42"), Some("x.com"));
        assert_eq!(detect_site("https://www.x.com/user/status/42"), Some("x.com"));
        assert_eq!(
            detect_site("https://www.instagram.com/reels/DZxVqbsTzqZ/"),
            Some("instagram.com")
        );
        assert_eq!(
            detect_site("https://www.pornhub.com/view_video.php?viewkey=abc"),
            Some("pornhub.com")
        );
    }

    #[test]
    fn hosts_that_merely_contain_a_site_name_are_not_that_site() {
        // The bug a substring test would have: both of these end in "x.com".
        assert_eq!(detect_site("https://www.netflix.com/watch/80100172"), None);
        assert_eq!(detect_site("https://www.vox.com/videos/123"), None);
        assert!(!is_site("https://www.netflix.com/watch/80100172", "x.com"));
    }

    #[test]
    fn a_site_name_in_the_path_or_query_does_not_count() {
        assert_eq!(
            detect_site("https://example.com/redirect?to=https://x.com/user/status/42"),
            None
        );
        assert!(!is_site("https://example.com/x.com/status/42", "x.com"));
    }

    #[test]
    fn subdomains_belong_to_their_site() {
        assert!(is_site("https://mobile.x.com/user/status/42", "x.com"));
        assert_eq!(
            detect_site("https://cdn.pornhub.com/video.mp4"),
            Some("pornhub.com")
        );
    }

    #[test]
    fn scheme_less_and_uppercase_input_still_resolves() {
        assert_eq!(detect_site("x.com/user/status/42"), Some("x.com"));
        assert_eq!(detect_site("  https://X.COM/user/status/42  "), Some("x.com"));
    }

    #[test]
    fn unparseable_input_belongs_to_no_site() {
        assert_eq!(detect_site(""), None);
        assert_eq!(detect_site("   "), None);
        assert!(!is_site("not a url", "x.com"));
    }
}
