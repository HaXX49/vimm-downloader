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

/// A streaming response body returned by [`VimmClient::post_stream`].
///
/// The client retries on a 5xx *status* before constructing this value.
/// Once returned, mid-stream I/O errors are the caller's responsibility
/// (issue #7 handles them via a resumable `.7z.tmp`).
pub struct StreamingResponse {
    /// `Content-Length` of the body, if the server sent it.
    pub content_length: Option<u64>,
    body: Option<reqwest::Response>,
}

impl StreamingResponse {
    /// Pull the next chunk of bytes from the body.
    ///
    /// Returns `Ok(None)` once the body is fully consumed (and on all
    /// subsequent calls).
    ///
    /// # Errors
    ///
    /// Returns [`VimmError::Http`] if reading the next chunk fails.
    pub async fn next_chunk(&mut self) -> Result<Option<Bytes>, VimmError> {
        match &mut self.body {
            Some(resp) => match resp.chunk().await {
                Ok(Some(chunk)) => Ok(Some(chunk)),
                Ok(None) => {
                    self.body = None;
                    Ok(None)
                }
                Err(e) => Err(VimmError::from(e)),
            },
            None => Ok(None),
        }
    }
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

    /// POST form fields to `url` and return the response as a stream.
    ///
    /// Retries on 5xx status (and retryable network errors) up to
    /// `1 + max_retries` total attempts, then hands the 2xx body to the
    /// caller via [`StreamingResponse`]. The `url` is used verbatim (the
    /// download host `dl3.vimm.net` differs from `base_url`).
    ///
    /// # Errors
    ///
    /// - [`VimmError::Http`] on a 4xx status, after retries are exhausted on
    ///   a 5xx, or on a non-retryable transport error.
    pub async fn post_stream(
        &self,
        url: &str,
        form: &[(&str, &str)],
    ) -> Result<StreamingResponse, VimmError> {
        let mut attempt: u32 = 0;
        loop {
            self.enforce_rate_limit().await;
            match self.http.post(url).form(form).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_server_error() && attempt < self.config.max_retries {
                        attempt += 1;
                        self.backoff(attempt).await;
                        continue;
                    }
                    let resp = resp.error_for_status().map_err(VimmError::from)?;
                    let content_length = resp.content_length();
                    return Ok(StreamingResponse {
                        content_length,
                        body: Some(resp),
                    });
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
    use wiremock::matchers::{method, path};
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

    #[tokio::test]
    async fn rate_limit_enforces_min_interval() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;

        let cfg = ClientConfig {
            base_url: server.uri(),
            min_request_interval: Duration::from_millis(120),
            ..ClientConfig::default()
        };
        let client = VimmClient::with_config(cfg).unwrap();

        let start = Instant::now();
        client.get_text("/a").await.unwrap();
        client.get_text("/b").await.unwrap();
        let elapsed = start.elapsed();

        // Two request starts must be >= 120ms apart.
        assert!(elapsed >= Duration::from_millis(120), "elapsed {elapsed:?}");
    }

    #[tokio::test]
    async fn cookies_persist_across_requests() {
        let server = MockServer::start().await;
        // Request 1: server sets a cookie.
        Mock::given(method("GET"))
            .and(path("/set"))
            .respond_with(
                ResponseTemplate::new(200).append_header("set-cookie", "sid=abc; Path=/"),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;
        // Request 2: expect the cookie to be sent back.
        Mock::given(method("GET"))
            .and(path("/echo"))
            .respond_with(ResponseTemplate::new(200).set_body_string("echo"))
            .mount(&server)
            .await;

        let client = VimmClient::with_config(fast_cfg(server.uri())).unwrap();
        client.get_text("/set").await.unwrap();
        client.get_text("/echo").await.unwrap();

        // Inspect received requests on the mock server.
        let received = server.received_requests().await.unwrap();
        let echo_req = received
            .iter()
            .find(|r| r.url.path() == "/echo")
            .expect("echo request received");
        let cookie_header = echo_req
            .headers
            .iter()
            .find_map(|(k, v)| {
                if k.as_str().eq_ignore_ascii_case("cookie") {
                    v.to_str().ok()
                } else {
                    None
                }
            })
            .unwrap_or("");
        assert!(
            cookie_header.contains("sid=abc"),
            "cookie header: {cookie_header}"
        );
    }

    #[tokio::test]
    async fn post_stream_returns_body_and_content_length() {
        let server = MockServer::start().await;
        let body = b"ROMBYTES".to_vec();
        Mock::given(method("POST"))
            .and(path("/dl"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(body.clone())
                    .insert_header("content-length", body.len().to_string()),
            )
            .mount(&server)
            .await;

        let client = VimmClient::with_config(fast_cfg(server.uri())).unwrap();
        let url = format!("{}/dl", server.uri());
        let mut resp = client
            .post_stream(&url, &[("mediaId", "1"), ("alt", "0")])
            .await
            .unwrap();
        assert_eq!(resp.content_length, Some(body.len() as u64));

        let mut collected = Vec::new();
        while let Some(chunk) = resp.next_chunk().await.unwrap() {
            collected.extend_from_slice(&chunk);
        }
        assert_eq!(collected, body);
        // Stream is exhausted; further reads yield None.
        assert!(resp.next_chunk().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn post_stream_retries_5xx_on_status() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/dl"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/dl"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok".to_vec()))
            .mount(&server)
            .await;

        let client = VimmClient::with_config(fast_cfg(server.uri())).unwrap();
        let url = format!("{}/dl", server.uri());
        let mut resp = client
            .post_stream(&url, &[("mediaId", "9")])
            .await
            .unwrap();
        let mut collected = Vec::new();
        while let Some(chunk) = resp.next_chunk().await.unwrap() {
            collected.extend_from_slice(&chunk);
        }
        assert_eq!(collected, b"ok");
    }
}
