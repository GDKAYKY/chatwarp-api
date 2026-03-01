use std::time::{Duration, Instant};

use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaWebVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl WaWebVersion {
    pub const fn fallback() -> Self {
        // Snapshot aligned with the bundled Baileys source in example/.
        Self {
            major: 2,
            minor: 3000,
            patch: 1033846690,
        }
    }
}

#[derive(Debug)]
struct CachedVersion {
    version: WaWebVersion,
    at: Instant,
}

/// Fetches and caches WA Web versions with resilient fallbacks.
#[derive(Debug)]
pub struct WaVersionManager {
    client: reqwest::Client,
    cache_ttl: Duration,
    inner: Mutex<Option<CachedVersion>>,
}

impl Default for WaVersionManager {
    fn default() -> Self {
        Self::new(Duration::from_secs(6 * 60 * 60))
    }
}

impl WaVersionManager {
    pub fn new(cache_ttl: Duration) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
            ),
        );
        headers.insert("sec-fetch-site", HeaderValue::from_static("none"));

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .expect("reqwest client must build");

        Self {
            client,
            cache_ttl,
            inner: Mutex::new(None),
        }
    }

    pub async fn get_version(&self) -> WaWebVersion {
        let mut guard = self.inner.lock().await;
        if let Some(cached) = guard.as_ref() {
            if cached.at.elapsed() < self.cache_ttl {
                return cached.version;
            }
        }

        let resolved = self
            .fetch_latest()
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(error = %error, "failed to fetch wa web version, using fallback");
                WaWebVersion::fallback()
            });

        *guard = Some(CachedVersion {
            version: resolved,
            at: Instant::now(),
        });

        resolved
    }

    pub async fn invalidate(&self) {
        let mut guard = self.inner.lock().await;
        *guard = None;
    }

    async fn fetch_latest(&self) -> Result<WaWebVersion, String> {
        let html = self
            .client
            .get("https://web.whatsapp.com")
            .send()
            .await
            .map_err(|error| error.to_string())?
            .text()
            .await
            .map_err(|error| error.to_string())?;

        if let Some(version) = extract_version_from_html(&html) {
            return Ok(version);
        }

        let sw_js = self
            .client
            .get("https://web.whatsapp.com/sw.js")
            .send()
            .await
            .map_err(|error| error.to_string())?
            .text()
            .await
            .map_err(|error| error.to_string())?;

        extract_version_from_sw_js(&sw_js).ok_or_else(|| "client_revision not found".to_owned())
    }
}

pub fn extract_version_from_html(html: &str) -> Option<WaWebVersion> {
    static CLIENT_REVISION_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let client_revision_re = CLIENT_REVISION_RE.get_or_init(|| {
        Regex::new(r#"client_revision\"?\s*:\s*(\d+)"#).expect("valid regex")
    });

    if let Some(caps) = client_revision_re.captures(html) {
        let patch = caps.get(1)?.as_str().parse::<u32>().ok()?;
        return Some(WaWebVersion {
            major: 2,
            minor: 3000,
            patch,
        });
    }

    static VERSION_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let version_re = VERSION_RE.get_or_init(|| {
        Regex::new(r#"\b(\d+)\.(\d+)\.(\d{6,})\b"#).expect("valid regex")
    });

    let caps = version_re.captures(html)?;
    Some(WaWebVersion {
        major: caps.get(1)?.as_str().parse::<u32>().ok()?,
        minor: caps.get(2)?.as_str().parse::<u32>().ok()?,
        patch: caps.get(3)?.as_str().parse::<u32>().ok()?,
    })
}

pub fn extract_version_from_sw_js(sw_js: &str) -> Option<WaWebVersion> {
    static CLIENT_REVISION_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let client_revision_re = CLIENT_REVISION_RE.get_or_init(|| {
        Regex::new(r#"client_revision\\?\"?\s*:\s*(\d+)"#).expect("valid regex")
    });

    let patch = client_revision_re
        .captures(sw_js)
        .and_then(|caps| caps.get(1))
        .and_then(|value| value.as_str().parse::<u32>().ok())?;

    Some(WaWebVersion {
        major: 2,
        minor: 3000,
        patch,
    })
}

#[cfg(test)]
mod tests {
    use super::{WaWebVersion, extract_version_from_html, extract_version_from_sw_js};

    #[test]
    fn parse_html_revision() {
        let html = r#"<html><body>{\"client_revision\":1033846690}</body></html>"#;
        assert_eq!(
            extract_version_from_html(html),
            Some(WaWebVersion {
                major: 2,
                minor: 3000,
                patch: 1033846690,
            })
        );
    }

    #[test]
    fn parse_html_semver_fallback() {
        let html = "<script>window.__WA_VERSION__='2.3000.1031111111';</script>";
        assert_eq!(
            extract_version_from_html(html),
            Some(WaWebVersion {
                major: 2,
                minor: 3000,
                patch: 1031111111,
            })
        );
    }

    #[test]
    fn parse_sw_js_revision() {
        let sw_js = r#"self.__WB_MANIFEST=[];var a={\"client_revision\":1032222222};"#;
        assert_eq!(
            extract_version_from_sw_js(sw_js),
            Some(WaWebVersion {
                major: 2,
                minor: 3000,
                patch: 1032222222,
            })
        );
    }
}
