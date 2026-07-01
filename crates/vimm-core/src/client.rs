//! Robust HTTP client for vimm.net.
//!
//! [`VimmClient`] wraps a `reqwest` client (rustls, cookies, gzip) with
//! retry/backoff and rate limiting. No `reqwest` types are exposed in the
//! public API.

use std::time::{Duration, Instant};

use bytes::Bytes;
use reqwest::Client as ReqwestClient;

use crate::error::VimmError;

/// Browser-like User-Agent (recent stable Chrome desktop).
pub const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

/// Default base URL for the Vault.
pub const DEFAULT_BASE_URL: &str = "https://vimm.net";

/// Default per-request timeout (30s).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Default minimum gap between successive request starts (500ms).
pub const DEFAULT_MIN_INTERVAL: Duration = Duration::from_millis(500);

/// Default number of retries beyond the first attempt (2 → 3 total, per DESIGN).
pub const DEFAULT_MAX_RETRIES: u32 = 2;

/// Client-level configuration for [`VimmClient`].
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Base URL prepended to `get_text` paths (no trailing slash).
    pub base_url: String,
    /// `User-Agent` header sent on every request.
    pub user_agent: String,
    /// Per-request timeout.
    pub timeout: Duration,
    /// Minimum gap between successive request starts.
    pub min_request_interval: Duration,
    /// Retries beyond the first attempt (total attempts = `1 + max_retries`).
    pub max_retries: u32,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            user_agent: DEFAULT_USER_AGENT.to_string(),
            timeout: DEFAULT_TIMEOUT,
            min_request_interval: DEFAULT_MIN_INTERVAL,
            max_retries: DEFAULT_MAX_RETRIES,
        }
    }
}

/// Async HTTP client for vimm.net with cookies, retry, and rate limiting.
pub struct VimmClient {
    http: ReqwestClient,
    config: ClientConfig,
    last_request: tokio::sync::Mutex<Option<Instant>>,
}

impl VimmClient {
    /// Build a client with default configuration.
    ///
    /// # Errors
    ///
    /// Returns [`VimmError::Http`] only if the underlying `reqwest::Client`
    /// fails to build (e.g. invalid TLS configuration).
    pub fn new() -> Result<Self, VimmError> {
        Self::with_config(ClientConfig::default())
    }

    /// Build a client from the given [`ClientConfig`].
    ///
    /// # Errors
    ///
    /// Returns [`VimmError::Http`] if the `reqwest::Client` cannot be built
    /// (e.g. invalid TLS configuration or a malformed `user_agent`).
    pub fn with_config(config: ClientConfig) -> Result<Self, VimmError> {
        let http = ReqwestClient::builder()
            .user_agent(config.user_agent.clone())
            .cookie_store(true)
            .timeout(config.timeout)
            .gzip(true)
            .build()
            .map_err(VimmError::from)?;
        Ok(Self {
            http,
            config,
            last_request: tokio::sync::Mutex::new(None),
        })
    }

    /// GET `base_url + path` and return the body as text.
    ///
    /// Retries on 5xx responses and retryable network errors (timeout,
    /// connect) up to `1 + max_retries` total attempts with exponential
    /// backoff. Does not retry 4xx responses.
    ///
    /// # Errors
    ///
    /// - [`VimmError::Http`] on a non-retryable HTTP status (4xx) or after
    ///   retries are exhausted on a 5xx, or on a non-retryable transport
    ///   error.
    pub async fn get_text(&self, path: &str) -> Result<String, VimmError> {
        let url = join_url(&self.config.base_url, path);
        let mut attempt: u32 = 0;
        loop {
            self.enforce_rate_limit().await;
            match self.http.get(&url).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_server_error() && attempt < self.config.max_retries {
                        attempt += 1;
                        self.backoff(attempt).await;
                        continue;
                    }
                    let resp = resp.error_for_status().map_err(VimmError::from)?;
                    let text = resp.text().await.map_err(VimmError::from)?;
                    return Ok(text);
                }
                Err(e) => {
                    if is_retryable(&e) && attempt < self.config.max_retries {
                        attempt += 1;
                        self.backoff(attempt).await;
                        continue;
                    }
                    return Err(VimmError::from(e));
                }
            }
        }
    }

    /// Sleep as needed so successive request starts are at least
    /// `min_request_interval` apart.
    async fn enforce_rate_limit(&self) {
        let mut guard = self.last_request.lock().await;
        if let Some(last) = *guard {
            let elapsed = last.elapsed();
            if elapsed < self.config.min_request_interval {
                tokio::time::sleep(self.config.min_request_interval - elapsed).await;
            }
        }
        *guard = Some(Instant::now());
    }

    /// Exponential backoff: `500ms * 2^(attempt-1)`, capped at `2^5`.
    async fn backoff(&self, attempt: u32) {
        let exp = attempt.saturating_sub(1).min(5);
        let multiplier = 1u64 << exp;
        let delay = Duration::from_millis(500) * multiplier as u32;
        tokio::time::sleep(delay).await;
    }
}

/// Whether a `reqwest` error is worth retrying (transient failures only).
fn is_retryable(e: &reqwest::Error) -> bool {
    e.is_timeout() || e.is_connect()
}

/// Join a base URL and a path into a single URL string.
fn join_url(base: &str, path: &str) -> String {
    let trimmed = base.trim_end_matches('/');
    if path.starts_with('/') {
        format!("{trimmed}{path}")
    } else {
        format!("{trimmed}/{path}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn default_config_matches_design() {
        let c = ClientConfig::default();
        assert_eq!(c.base_url, "https://vimm.net");
        assert_eq!(c.min_request_interval, Duration::from_millis(500));
        assert_eq!(c.max_retries, 2);
        assert_eq!(c.timeout, Duration::from_secs(30));
        assert!(c.user_agent.contains("Chrome"));
    }

    #[test]
    fn client_constructs_with_defaults() {
        let _client = VimmClient::new().expect("default client builds");
    }

    /// Build a config pointed at a mock server with a tiny rate-limit interval.
    fn fast_cfg(base_url: String) -> ClientConfig {
        ClientConfig {
            base_url,
            min_request_interval: Duration::from_millis(1),
            ..ClientConfig::default()
        }
    }

    #[tokio::test]
    async fn get_text_returns_body_on_200() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("hello vault"))
            .mount(&server)
            .await;

        let client = VimmClient::with_config(fast_cfg(server.uri())).unwrap();
        let body = client.get_text("/vault").await.unwrap();
        assert_eq!(body, "hello vault");
    }

    #[tokio::test]
    async fn get_text_retries_5xx_then_succeeds() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500))
            .up_to_n_times(2)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;

        let client = VimmClient::with_config(fast_cfg(server.uri())).unwrap();
        let body = client.get_text("/x").await.unwrap();
        assert_eq!(body, "ok");
    }

    #[tokio::test]
    async fn get_text_does_not_retry_4xx() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client = VimmClient::with_config(fast_cfg(server.uri())).unwrap();
        let err = client.get_text("/missing").await.unwrap_err();
        assert!(matches!(err, VimmError::Http(_)));
    }
}
