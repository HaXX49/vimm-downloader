# Issue #3 — Robust HTTP Client Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
> Branch: `issue-3/http-client`. Spec: `docs/superpowers/specs/2026-07-01-http-client-design.md`.

**Goal:** Implement `VimmClient` (async, reqwest+rustls, cookies, retry/backoff, rate limiting) in `crates/vimm-core/src/client.rs` with offline wiremock tests.

**Architecture:** One new module `client.rs` exposing `ClientConfig`, `VimmClient`, `StreamingResponse`. No `reqwest` types leak into the public API. Retry on 5xx + retryable network errors; rate limit via `Mutex<Option<Instant>>` + `tokio::time::sleep`; `post_stream` retries on status then hands a `StreamingResponse` to the caller (#7 owns mid-stream I/O).

**Tech Stack:** reqwest 0.13 (rustls, gzip, stream, cookies), tokio, bytes, thiserror, wiremock 0.6 (dev).

## Global Constraints

- `#![forbid(unsafe_code)]`, `#![warn(missing_docs)]`, `#![warn(clippy::all, clippy::pedantic)]` already set in `vimm-core/src/lib.rs` — every public item needs a doc comment; pedantic lints must pass (`-D warnings`).
- CI gates: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features --no-fail-fast`.
- rustls-only (no native-tls). Cookies enabled.
- Commit after each task. Do not push until the end.

---

### Task 1: Dependencies + module wiring

**Files:**
- Modify: `Cargo.toml` (workspace deps: add `cookies` to reqwest, add `bytes`)
- Modify: `crates/vimm-core/Cargo.toml` (add `bytes` dep, add `[dev-dependencies]` wiremock)
- Create: `crates/vimm-core/src/client.rs` (module doc + empty, just enough to compile)
- Modify: `crates/vimm-core/src/lib.rs` (declare `pub mod client;`)

- [ ] **Step 1: Add `cookies` to workspace reqwest + add `bytes`**

In `Cargo.toml`, change line 19 and add a `bytes` line after the `tokio` line (HTTP/async block):

```toml
[workspace.dependencies]
# HTTP / async
reqwest = { version = "0.13", default-features = false, features = ["rustls", "gzip", "stream", "cookies"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "fs", "io-util"] }
bytes = "1"
url = "2"
```

- [ ] **Step 2: Add `bytes` + wiremock to `vimm-core`**

In `crates/vimm-core/Cargo.toml`, add `bytes` to `[dependencies]` (after `reqwest`) and add a `[dev-dependencies]` section at the end:

```toml
[dependencies]
reqwest = { workspace = true }
bytes = { workspace = true }
tokio = { workspace = true }
url = { workspace = true }
scraper = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
base64 = { workspace = true }
regex = { workspace = true }
sevenz-rust2 = { workspace = true }
zip = { workspace = true }
thiserror = { workspace = true }
toml = { workspace = true }
dirs = { workspace = true }

[dev-dependencies]
wiremock = "0.6"
```

- [ ] **Step 3: Create `client.rs` stub**

Create `crates/vimm-core/src/client.rs`:

```rust
//! Robust HTTP client for vimm.net.
//!
//! [`VimmClient`] wraps a `reqwest` client (rustls, cookies, gzip) with
//! retry/backoff and rate limiting. No `reqwest` types are exposed in the
//! public API.
```

- [ ] **Step 4: Declare the module in `lib.rs`**

In `crates/vimm-core/src/lib.rs`, add `pub mod client;` after `pub mod model;` (line 12). Do NOT re-export types yet (Task 7).

```rust
pub mod error;
pub mod model;
pub mod client;
```

- [ ] **Step 5: Verify it builds**

Run: `cargo build -p vimm-core`
Expected: compiles (warning about unused module is fine for now; or none).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/vimm-core/Cargo.toml crates/vimm-core/src/client.rs crates/vimm-core/src/lib.rs
git commit -m "build(core): wire client module + cookies/bytes/wiremock deps"
```

---

### Task 2: ClientConfig + VimmClient construction (TDD)

**Files:**
- Modify: `crates/vimm-core/src/client.rs` (add config + client struct + new/with_config)
- Test: inline `#[cfg(test)] mod tests` in `client.rs`

**Interfaces:**
- Produces: `ClientConfig` (with `Default`), `VimmClient::new()`, `VimmClient::with_config(ClientConfig)`.

- [ ] **Step 1: Write the failing test**

Append to `crates/vimm-core/src/client.rs`:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vimm-core client::tests`
Expected: FAIL to compile — `VimmClient::new` not defined.

- [ ] **Step 3: Implement `new` / `with_config`**

Append to `crates/vimm-core/src/client.rs` (in the non-test part):

```rust
impl VimmClient {
    /// Build a client with default configuration.
    ///
    /// Errors only if the underlying `reqwest::Client` fails to build
    /// (e.g. bad TLS configuration).
    pub fn new() -> Result<Self, VimmError> {
        Self::with_config(ClientConfig::default())
    }

    /// Build a client from the given [`ClientConfig`].
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vimm-core client::tests`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vimm-core/src/client.rs
git commit -m "feat(core): add ClientConfig and VimmClient construction"
```

---

### Task 3: `get_text` — basic, retry on 5xx, no retry on 4xx (TDD)

**Files:**
- Modify: `crates/vimm-core/src/client.rs` (add `get_text`, `enforce_rate_limit`, `backoff`, `is_retryable`)

**Interfaces:**
- Produces: `pub async fn get_text(&self, path: &str) -> Result<String, VimmError>`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `client.rs`:

```rust
    use wiremock::Mock;
    use wiremock::matchers::method;
    use wiremock::ResponseTemplate;

    fn fast_cfg(base_url: String) -> ClientConfig {
        ClientConfig {
            base_url,
            min_request_interval: Duration::from_millis(1),
            ..ClientConfig::default()
        }
    }

    #[tokio::test]
    async fn get_text_returns_body_on_200() {
        let server = wiremock::MockServer::start().await;
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
        let server = wiremock::MockServer::start().await;
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
        let server = wiremock::MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client = VimmClient::with_config(fast_cfg(server.uri())).unwrap();
        let err = client.get_text("/missing").await.unwrap_err();
        assert!(matches!(err, VimmError::Http(_)));
    }
```

(Add `use wiremock::MockServer;` to the test module imports, or use fully-qualified `wiremock::MockServer::start()`.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vimm-core client::tests`
Expected: FAIL to compile — `get_text` not defined.

- [ ] **Step 3: Implement `get_text` + helpers**

Append to the non-test part of `client.rs`:

```rust
impl VimmClient {
    /// GET `base_url + path` and return the body as text.
    ///
    /// Retries on 5xx and retryable network errors (timeout, connect)
    /// up to `1 + max_retries` total attempts with exponential backoff.
    /// Does NOT retry 4xx.
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

    /// Exponential backoff: `500ms * 2^(attempt-1)` (capped at 2^5).
    async fn backoff(&self, attempt: u32) {
        let exp = attempt.saturating_sub(1).min(5);
        let multiplier = 1u64 << exp;
        let delay = Duration::from_millis(500) * multiplier as u32;
        tokio::time::sleep(delay).await;
    }
}

/// Whether a `reqwest` error is worth retrying.
fn is_retryable(e: &reqwest::Error) -> bool {
    e.is_timeout() || e.is_connect()
}

/// Join a base URL and a path into one URL string.
fn join_url(base: &str, path: &str) -> String {
    let trimmed = base.trim_end_matches('/');
    if path.starts_with('/') {
        format!("{trimmed}{path}")
    } else {
        format!("{trimmed}/{path}")
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vimm-core client::tests`
Expected: PASS (5 tests: 2 from Task 2 + 3 here). The retry test takes ~1.5s due to backoff (500ms + 1s) — acceptable.

- [ ] **Step 5: Commit**

```bash
git add crates/vimm-core/src/client.rs
git commit -m "feat(core): implement get_text with retry/backoff and rate limiting"
```

---

### Task 4: Rate-limit timing test (TDD)

**Files:**
- Modify: `crates/vimm-core/src/client.rs` (add timing test)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module:

```rust
    #[tokio::test]
    async fn rate_limit_enforces_min_interval() {
        let server = wiremock::MockServer::start().await;
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
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p vimm-core client::tests::rate_limit_enforces_min_interval`
Expected: PASS (rate limiting already implemented in Task 3; this pins the behavior).

- [ ] **Step 3: Commit**

```bash
git add crates/vimm-core/src/client.rs
git commit -m "test(core): pin rate-limit min-interval behavior"
```

---

### Task 5: Cookie round-trip test (TDD)

**Files:**
- Modify: `crates/vimm-core/src/client.rs` (add cookie test)

- [ ] **Step 1: Write the test**

Add to the `tests` module:

```rust
    #[tokio::test]
    async fn cookies_persist_across_requests() {
        let server = wiremock::MockServer::start().await;
        // Request 1: server sets a cookie.
        Mock::given(method("GET").and(wiremock::matchers::path("/set")))
            .respond_with(
                ResponseTemplate::new(200)
                    .append_header("set-cookie", "sid=abc; Path=/"),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;
        // Request 2: expect the cookie to be sent back.
        Mock::given(method("GET").and(wiremock::matchers::path("/echo")))
            .respond_with(ResponseTemplate::new(200).set_body_string("echo"))
            .mount(&server)
            .await;

        let client = VimmClient::with_config(fast_cfg(server.uri())).unwrap();
        client.get_text("/set").await.unwrap();

        // Inspect received requests on the mock server.
        let received = server.received_requests().await.unwrap();
        let echo_req = received
            .iter()
            .find(|r| r.url.path() == "/echo")
            .expect("echo request received");
        let cookie_header = echo_req
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("cookie"))
            .map(|(_, v)| v.as_str())
            .unwrap_or("");
        assert!(cookie_header.contains("sid=abc"), "cookie header: {cookie_header}");
    }
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p vimm-core client::tests::cookies_persist_across_requests`
Expected: PASS (cookie store enabled in Task 2).

- [ ] **Step 3: Commit**

```bash
git add crates/vimm-core/src/client.rs
git commit -m "test(core): verify cookie store persists across requests"
```

---

### Task 6: `post_stream` + `StreamingResponse` (TDD)

**Files:**
- Modify: `crates/vimm-core/src/client.rs` (add `StreamingResponse` + `post_stream`)

**Interfaces:**
- Produces: `pub struct StreamingResponse { content_length: Option<u64> }` with `pub async fn next_chunk(&mut self) -> Result<Option<Bytes>, VimmError>`; `pub async fn post_stream(&self, url: &str, form: &[(&str, &str)]) -> Result<StreamingResponse, VimmError>`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module:

```rust
    #[tokio::test]
    async fn post_stream_returns_body_and_content_length() {
        let server = wiremock::MockServer::start().await;
        let body = b"ROMBYTES".to_vec();
        Mock::given(method("POST").and(wiremock::matchers::path("/dl")))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(body.clone())
                    .insert_header("content-length", body.len().to_string()),
            )
            .mount(&server)
            .await;

        let client = VimmClient::with_config(fast_cfg(server.uri())).unwrap();
        let url = format!("{}/dl", server.uri());
        let mut resp = client.post_stream(&url, &[("mediaId", "1"), ("alt", "0")]).await.unwrap();
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
        let server = wiremock::MockServer::start().await;
        Mock::given(method("POST").and(wiremock::matchers::path("/dl")))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST").and(wiremock::matchers::path("/dl")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok".to_vec()))
            .mount(&server)
            .await;

        let client = VimmClient::with_config(fast_cfg(server.uri())).unwrap();
        let url = format!("{}/dl", server.uri());
        let mut resp = client.post_stream(&url, &[("mediaId", "9")]).await.unwrap();
        let mut collected = Vec::new();
        while let Some(chunk) = resp.next_chunk().await.unwrap() {
            collected.extend_from_slice(&chunk);
        }
        assert_eq!(collected, b"ok");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vimm-core client::tests`
Expected: FAIL to compile — `StreamingResponse` and `post_stream` not defined.

- [ ] **Step 3: Implement `StreamingResponse` + `post_stream`**

Add to the non-test part of `client.rs` (before the `impl VimmClient` block, after the `VimmClient` struct):

```rust
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
```

Add `post_stream` to the `impl VimmClient` block:

```rust
    /// POST form fields to `url` and return the response as a stream.
    ///
    /// Retries on 5xx status (and retryable network errors) up to
    /// `1 + max_retries` total attempts, then hands the 2xx body to the
    /// caller via [`StreamingResponse`].
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
                    Ok(StreamingResponse {
                        content_length,
                        body: Some(resp),
                    })
                }
                Err(e) => {
                    if is_retryable(&e) && attempt < self.config.max_retries {
                        attempt += 1;
                        self.backoff(attempt).await;
                        continue;
                    }
                    Err(VimmError::from(e))
                }
            }
        }
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vimm-core client::tests`
Expected: PASS (all client tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vimm-core/src/client.rs
git commit -m "feat(core): implement post_stream + StreamingResponse"
```

---

### Task 7: Re-exports + full gate (fmt/clippy/test)

**Files:**
- Modify: `crates/vimm-core/src/lib.rs` (re-export `VimmClient`, `ClientConfig`, `StreamingResponse`)
- Modify: `crates/vimm-bindings/src/lib.rs` (re-export the same for FFI surface consistency)

- [ ] **Step 1: Re-export from `vimm-core`**

In `crates/vimm-core/src/lib.rs`, add a `pub use client::…` line:

```rust
pub mod error;
pub mod model;
pub mod client;

pub use error::VimmError;
pub use model::{
    ExtraFlag, Format, GameDetail, GameSummary, Media, Op, Order, Ratings, SearchMode, SearchQuery,
    Sort, System,
};
pub use client::{ClientConfig, StreamingResponse, VimmClient};
```

- [ ] **Step 2: Re-export from `vimm-bindings` (consistency)**

In `crates/vimm-bindings/src/lib.rs`, add the three new types to the existing `pub use vimm_core::{…}`:

```rust
pub use vimm_core::{
    ClientConfig, ExtraFlag, Format, GameDetail, GameSummary, Media, Op, Order, Ratings,
    SearchMode, SearchQuery, Sort, StreamingResponse, System, VimmClient, VimmError,
};
```

- [ ] **Step 3: Format check**

Run: `cargo fmt --all -- --check`
Expected: exit 0. If it reports changes, run `cargo fmt --all` and re-check.

- [ ] **Step 4: Clippy with warnings as errors**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: exit 0. Watch for: `missing_docs` on new public items (add doc comments), `must_use` suggestions, pedantic nits (e.g. `clippy::must_use` on constructors — add `#[must_use]` to `new`/`with_config` if flagged).

- [ ] **Step 5: Full test suite**

Run: `cargo test --all-features --no-fail-fast`
Expected: all pass (6 model tests + ~8 client tests).

- [ ] **Step 6: Commit**

```bash
git add crates/vimm-core/src/lib.rs crates/vimm-bindings/src/lib.rs
git commit -m "feat(core): re-export VimmClient, ClientConfig, StreamingResponse"
```

---

## Self-Review

- **Spec coverage:** ClientConfig (Task 2) ✓ · VimmClient new/with_config (2) ✓ · get_text (3) ✓ · retry 5xx (3) ✓ · no-retry 4xx (3) ✓ · rate limit (3 impl, 4 test) ✓ · cookies (2 impl, 5 test) ✓ · post_stream + StreamingResponse (6) ✓ · re-exports (7) ✓ · deps cookies/bytes/wiremock (1) ✓.
- **Placeholder scan:** No TBD/TODO; all code blocks concrete. The only "verify" steps have exact commands + expected output. ✔
- **Type consistency:** `StreamingResponse::next_chunk -> Result<Option<Bytes>, VimmError>` matches across Task 6 test + impl. `post_stream` signature matches spec. `ClientConfig` fields match spec. ✔
- **Test design:** Tests use `fast_cfg` (1ms interval) to keep most tests sub-second; the retry test accepts ~1.5s backoff. Rate-limit test uses 120ms to be measurable. Cookie test inspects `server.received_requests()`. ✔
