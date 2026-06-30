//! vimm-bindings: UniFFI facade for `vimm-core`.
//!
//! v2 scope: generate Swift/Kotlin/Python bindings from this crate.
//! v1 ships only a compiling stub that re-exports the core types so the
//! workspace builds end-to-end and the FFI surface can grow incrementally.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::doc_markdown)]

pub use vimm_core::{
    ExtraFlag, Format, GameDetail, GameSummary, Media, Op, Order, SearchMode, SearchQuery, Sort,
    System, VimmError,
};
