# Issue #3 — Robust HTTP Client (VimmClient) Design

> Status: **Approved** (2026-07-01). Branch: `issue-3/http-client`.
> This spec pins only the decisions not already locked in `DESIGN.md`. For
> the data model, scraping specifics, and dependency philosophy, see
> `DESIGN.md` (sections "Pragmatic robustness", "Dependencies", issue #3).

## Goal

`crates/vimm-core/src/client.rs` — an async, reqwest-backed (rustls) HTTP
client for vimm.net that is polite, resilient, and portable. It is the
foundation for #4 (systems), #5 (search), #6 (detail), and #7 (download).

## Public API (no `reqwest` types leak)

`DESIGN.md` mandates a clean public API for the future UniFFI FFI surface
(`vimm-bindings`). Therefore `reqwest::Response` is never returned; the
client owns reqwest internally and exposes core-owned types.

```rust
pub struct ClientConfig {
    pub base_url: String,             // default "https://vimm.net"
    pub user_agent: String,           // default: a current Chrome desktop UA
    pub timeout: Duration,            // default 30s
    pub min_request_interval: Duration, // default 500ms
    pub max_retries: u32,             // default 2 (== 3 total attempts, per DESIGN)
}
impl Default for ClientConfig;

pub struct VimmClient { /* reqwest::Client, ClientConfig, last_request: Instant */ }
impl VimmClient {
    pub fn new() -> Result<Self, VimmError>;
    pub fn with_config(cfg: ClientConfig) -> Result<Self, VimmError>;
    pub async fn get_text(&self, path: &str) -> Result<String, VimmError>;
    pub async fn post_stream(
        &self,
        url: &str,
        form: &[(&str, &str)],
    ) -> Result<StreamingResponse, VimmError>;
}

pub struct StreamingResponse {
    pub content_length: Option<u64>,
    // private: reqwest body stream, mapped to VimmError
}
impl StreamingResponse {
    pub async fn next_chunk(&mut self) -> Result<Option<bytes::Bytes>, VimmError>;
}
```

`max_retries` semantics: total attempts = `1 + max_retries`. DESIGN says "3
attempts", so **default `max_retries = 2`** (== 3 total attempts).

## Behavior

- **Transport**: browser User-Agent, `cookie_store(true)`, rustls, gzip,
  redirect policy (default 10). `reqwest::Client` built once in `new()`/`with_config`.
- **Retry**: loop up to `1 + max_retries` total attempts. Retry when:
  - response status is 5xx, or
  - reqwest error is retryable (timeout, connect, request mid-connection).
  Do NOT retry on 4xx (client error) or non-retryable reqwest errors.
  Backoff: exponential, base 500ms × 2^attempt, optional jitter.
- **Rate limiting**: track `last_request: Instant`; before each request,
  `tokio::time::sleep` as needed to keep `min_request_interval` between
  successive request *starts*.
- **`post_stream` retry boundary**: retries on the response *status* (5xx).
  Once a 2xx body stream is returned, mid-stream I/O errors are the caller's
  concern (#7 handles them via resumable `.7z.tmp`). This keeps the client
  simple and avoids re-streaming whole downloads.
- **`get_text`**: GET `base_url + path`, return the decoded body as `String`.

## Dependencies

- Workspace `reqwest` features: **add `cookies`** (currently
  `["rustls","gzip","stream"]` in root `Cargo.toml:19`). DESIGN:122 requires
  cookies.
- `vimm-core` `[dev-dependencies]`: add `wiremock = "0.6"` for offline
  retry/rate-limit/cookie tests.
- `vimm-core` `[dependencies]`: add `bytes` (for `StreamingResponse` chunks).
  `tokio` (already present) provides `time::sleep`. `futures` not needed
  (poll `next_chunk` instead of exposing a `Stream`).

## Tests (wiremock, offline — no network)

1. `get_text` returns body on 200.
2. `get_text` retries: mock 500 ×2 then 200 → returns text, exactly 3 total
   requests.
3. `get_text` does not retry 4xx → returns `VimmError::Http` immediately, 1 request.
4. Rate limit: two sequential `get_text` calls; assert elapsed ≥
   `min_request_interval` (use a small interval like 50ms in the test).
5. Cookies: mock sets a cookie on request 1; request 2 (same client) sends
   it — assert via mock request inspection.
6. `post_stream`: mock 200 with a known body + `Content-Length`; assert
   `content_length` and that `next_chunk` yields the bytes then `None`.

## Out of scope (deferred)

- Actual `download()` to disk + indicatif progress → #7.
- `--base-url` / `-v` CLI wiring → #10.
- TOML config file + format preferences → #11.
- Live (`--features live`) integration tests → #14.

## Open items (resolve at impl time)

- Exact Chrome UA string (use a recent stable desktop Chrome UA).
- Whether jitter is worth the complexity; default to no jitter, add if cheap.
