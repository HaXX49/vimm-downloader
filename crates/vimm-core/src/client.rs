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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
