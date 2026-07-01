# vimm-downloader

[![CI](https://github.com/HaXX49/vimm-downloader/actions/workflows/ci.yml/badge.svg)](https://github.com/HaXX49/vimm-downloader/actions/workflows/ci.yml)

A portable downloader for the [Vimm's Lair Vault](https://vimm.net/vault) — browse
and download retro game ROMs from the command line.

Built around a pure, async Rust core (`vimm-core`) with a thin CLI frontend
(`vimm-downloader`). Designed for portability (iOS, Android, WASM via UniFFI in v2)
and static/cross-compilable builds (rustls-only — no OpenSSL dependency).

## Status

| Milestone | Issues | Status |
|---|---|---|
| **M1** Foundation & Scaffolding | Cargo workspace, data model, error types | ✅ Done |
| **M2** Scraping the Vault | HTTP client, systems / search / detail parsers | ✅ Done |
| **M3** Download Pipeline | Download streaming, 7z extraction, live spike | 🔜 Next |
| **M4** CLI & Config | Full clap structure, TOML config file | ⬜ Planned |
| **M5** Testing & Polish | Offline fixtures, synthetic tests, cross-compile | ⬜ Planned |

## Quick Start

```bash
# Build (rustls-only — no system deps needed)
cargo build --release

# List the 33 supported consoles and their slugs
./target/release/vimm-downloader systems
./target/release/vimm-downloader systems --json     # machine-readable output

# Search for games (parser ready, CLI wiring coming in M4)
# vimm-downloader search --query "mario" --system NES

# Show game detail (parser ready, CLI wiring coming in M4)
# vimm-downloader info 834

# Download a ROM (coming in M3)
# vimm-downloader download 834 --format rvz --out ~/roms
```

The `--json` flag is available globally.

## Architecture

```
vimm-downloader/
├── Cargo.toml                  # workspace
├── crates/
│   ├── vimm-core/              # rlib + cdylib + staticlib
│   │   └── src/{lib,model,error,client,systems,search,detail}.rs
│   ├── vimm-cli/               # binary: clap + indicatif
│   │   └── src/main.rs
│   └── vimm-bindings/          # UniFFI facade (stub for v2)
└── tests/fixtures/             # offline HTML fixtures
```

The core is pure, async Rust with a clean public API. The CLI consumes it directly;
the bindings crate wraps it with UniFFI macros for Swift/Kotlin/Python (v2).

### Modules

| `vimm-core/src/` | Purpose | Status |
|---|---|---|
| `model.rs` | Shared data types (`System`, `GameSummary`, `GameDetail`, `Media`, etc.) | ✅ |
| `error.rs` | Strongly-typed error taxonomy | ✅ |
| `client.rs` | Async HTTP client (reqwest+rustls, cookies, retry/backoff, rate limiting) | ✅ |
| `systems.rs` | Parse the 33 console slugs from `/vault` | ✅ |
| `search.rs` | Parse search results (per-system + all-system dual schema) | ✅ |
| `detail.rs` | Parse game detail pages (embedded JSON, base64 titles, selects) | ✅ |
| `download.rs` | Stream ROM to disk with progress | 🔜 |
| `archive.rs` | 7z extraction (by-elimination junk removal) | 🔜 |
| `config.rs` | TOML config (per-system format preferences) | 🔜 |

## CLI

```
vimm-downloader systems                                          [--json]
vimm-downloader search --query <q> [--system <slug>] [--region <r>]
               [--sort <field>] [--order ASC|DESC] [--limit <n>]
               [--players <n>] [--year <op>value] [--publisher <str>] [--json]
vimm-downloader info    <id>                                     [--json]
vimm-downloader download <id>
    [--version <ver>] [--disc <n>] [--format <key>]
    [--out <dir>] [--archive] [--keep-extras] [--config <path>]
    [-v] [--json]
```

The `systems` subcommand is fully wired. `search`/`info`/`download` parsers are
implemented in `vimm-core`; their CLI wiring is part of [issue #10](https://github.com/HaXX49/vimm-downloader/issues/10).

## Dependencies

**Transport:** reqwest (rustls, cookies, gzip) · **Runtime:** tokio · **Parsing:**
scraper, serde/serde_json, regex, base64 · **CLI:** clap, indicatif, anyhow ·
**Archive:** sevenz-rust2 (pure Rust 7z), zip (fallback) · **Config:** toml, dirs

**Zero OpenSSL.** The binary is statically linkable on Linux (musl), macOS, and
Windows with no system library dependencies.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --no-fail-fast      # offline — no network
cargo test --features live                     # +integration tests (manual)
```

CI runs on every push/PR (fmt + clippy + test).

### Design

See [`DESIGN.md`](./DESIGN.md) for the full locked specification, data model,
scraping specifics, download pipeline, and issue plan.

## License

MIT
