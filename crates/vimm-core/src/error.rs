//! Strongly-typed errors for `vimm-core`.
//!
//! The CLI converts these into `anyhow::Error` for user-facing output;
//! FFI bindings surface them as string error codes.

use thiserror::Error;

/// All errors produced by `vimm-core`.
#[derive(Debug, Error)]
pub enum VimmError {
    /// HTTP transport failure (network down, DNS, TLS, timeout exhausted).
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// Could not parse a fetched page (HTML structure changed, missing field,
    /// bad JSON in the embedded `media` array, etc.).
    #[error("parse failure: {0}")]
    Parse(String),

    /// I/O error (writing the downloaded archive to disk, extraction, etc.).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Archive extraction failure (corrupt 7z/zip, unsupported codec, etc.).
    #[error("archive error: {0}")]
    Archive(String),

    /// Config file missing, unreadable, or malformed TOML.
    #[error("config error: {0}")]
    Config(String),

    /// Base64 decode failure (the site base64-encodes `GoodTitle`).
    #[error("base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),

    /// JSON decode failure (the embedded `media` JS array).
    #[error("JSON decode error: {0}")]
    Json(#[from] serde_json::Error),

    /// A user-supplied input was invalid (unknown system slug, bad game ID, …).
    #[error("invalid input: {0}")]
    InvalidInput(String),
}
