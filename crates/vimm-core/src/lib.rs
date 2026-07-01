//! vimm-core: portable core library for the Vimm's Lair Vault downloader.
//!
//! Pure, async Rust. No frontend logic. Consumed by `vimm-cli` directly and
//! by `vimm-bindings` (UniFFI) for mobile/WASM frontends.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::doc_markdown)]

pub mod client;
pub mod error;
pub mod model;
pub mod search;
pub mod systems;

pub use client::{ClientConfig, StreamingResponse, VimmClient};
pub use error::VimmError;
pub use model::{
    ExtraFlag, Format, GameDetail, GameSummary, Media, Op, Order, Ratings, SearchMode, SearchQuery,
    Sort, System,
};
