//! Robust HTTP client for vimm.net.
//!
//! [`VimmClient`] wraps a `reqwest` client (rustls, cookies, gzip) with
//! retry/backoff and rate limiting. No `reqwest` types are exposed in the
//! public API.
