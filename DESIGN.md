# vimm-downloader — Design Document

A portable downloader for the [Vimm's Lair Vault](https://vimm.net/vault), built around a portable core with thin frontends.

## Goals

- **Portable core library** usable from CLI, mobile (iOS/Android), and web (WASM)
- **v1 frontend**: CLI only
- **v1 scope**: search (per-system + all-system), game detail, download with extraction
- **Pragmatic robustness**: browser-like UA, cookies, retry/backoff, rate limiting, rustls-only (static/cross-compilable)

## Architecture

```
vimm-downloader/
├── Cargo.toml                       # workspace
├── crates/
│   ├── vimm-core/                   # rlib + cdylib + staticlib
│   │   └── src/{lib,model,client,search,detail,download,archive,config,error}.rs
│   ├── vimm-cli/                    # binary: clap + indicatif
│   │   └── src/main.rs
│   └── vimm-bindings/               # UniFFI facade (stub for v2 iOS/Android)
└── tests/fixtures/*.html            # offline parser fixtures
```

The core is pure, async Rust with a clean public API; the CLI consumes it directly; the bindings crate wraps it with UniFFI macros so Swift/Kotlin/Python can be generated later without touching the core.

## Data model (multi-format aware)

```rust
struct System { slug, name, launch_year }            // 33 consoles
struct SearchQuery {
    system: Option<String>,   // None or "all" → mode=adv, system="" ; Some(slug) → mode=list
    q: String,
    players: Option<(Op, u8)>,
    simultaneous: Option<bool>,
    publisher: Option<String>,
    year: Option<(Op, u16)>,
    rating: Option<(Op, f32)>,   // only meaningful in per-system mode
    region: Option<String>,
    sort: Sort,                  // Title | Players | Year | Rating
    order: Order,                // ASC | DESC
    section: Option<String>,     // letter or "number"; per-system only
}
struct GameSummary {
    id: u32,
    title: String,
    system: String,              // always populated
    regions: Vec<String>,
    version: String,
    languages: Vec<String>,      // "-" → empty
    extras: Vec<ExtraFlag>,      // T/D/P/U/B from <b class="redBorder"> badges
    rating: Option<f32>,         // Some in per-system mode, None in all-system mode
}
struct GameDetail {
    id, system, title, region, players, year, publisher, serial,
    ratings, verified_date, media: Vec<Media>,
}
struct Media {
    id, version, disc, good_title, serial, verified_date,
    formats: Vec<Format>,        // ≥1
}
struct Format { key, label, description, alt: u8, zipped_size_bytes }
enum ExtraFlag { Translated, Demo, Prototype, Unlicensed, Bonus }
```

No checksum fields exposed in v1 (verification deferred).

## Scraping specifics

- **Systems**: parse `/vault` `#subMenu` links `/vault/{slug}` + `title="Launched …"` → slug, name, launch year.
- **Search**: `GET /vault/?p=list&…` → parse results `<table>`; skip decoy `/vault/999999` href; real `/vault/{id}`; region from `<img class="flag" title>`. Detect schema by first `<th>`: `System` → all-system (drops Rating), `Title` → per-system.
- **Detail**: `GET /vault/{id}` → extract `let media=[…]` JSON via regex + serde_json; decode base64 `GoodTitle`; build `formats` from `Mirror[]` + `Zipped/AltZipped/AltZipped2` by index; synthesize single format when `#dl_format` absent. Parse `#dl_version` (incl. `selected`), `#disc_number`, `#dl_format` options (label+title) and metadata `<table>`.

## Download pipeline (7z, extract by elimination)

```
POST dl3.vimm.net  mediaId={id}&alt={0|1|2}
  → stream to {out}/.{stem}.7z.tmp   (indicatif progress bar)
  │
  ├─ --archive: rename → {stem}.7z                         ✓ done
  └─ else: detect by magic bytes (7z primary, zip fallback)
           extract all entries to {out}/
           delete junk by blocklist (txt nfo diz jpg jpeg png html url)   ← "by elimination" keeps the ROM
           delete .7z.tmp                                   ✓ done (ROM kept)
--keep-extras → don't delete junk.   Final filename = 7z inner entry name (falls back to GoodTitle+ext).
```

No format-extension allowlist needed — the blocklist-by-elimination handles all 33 systems including `.bin`+`.cue`/`.gdi` CD images (companions aren't junk, so they survive).

## Defaults & config (v1)

- **Non-interactive defaults**: newest version (site's `selected`), disc 1, first `#dl_format` option (e.g. `.ciso`) — mirrors the website exactly.
- **Config** `~/.config/vimm-downloader/config.toml` (via `dirs` + `toml`):
  ```toml
  [formats]
  GameCube = "rvz"
  Wii = "ciso"
  ```
  Resolution order: CLI `--format` > config per-system > site default.
- **Interactive prompts**: deferred to v2 (`dialoguer`).

## CLI surface (v1)

```
vimm-downloader systems                                          [--json]
vimm-downloader search --query "armored core"                    # all systems
vimm-downloader search --query "mario" --system NES [--sort rating --order desc] [--limit 50]
                       [--region USA,Europe] [--year ">=1990"] [--players ">=2"] [--json]
vimm-downloader info    <id>                                     [--json]
vimm-downloader download <id>
    [--version <ver>] [--disc <n>] [--format <key>]
    [--out <dir>]            default: cwd
    [--archive]              keep raw 7z, skip extraction
    [--keep-extras]          keep .nfo/.txt etc.
    [--config <path>]        default: ~/.config/vimm-downloader/config.toml
    [-v] [--base-url <url>]
```

## Dependencies (portable, rustls-only, static/cross-compilable)

`reqwest`(rustls, cookies, gzip) · `tokio` · `scraper` · `serde`/`serde_json` · `base64` · `regex`/`url` · `sevenz-rust2` (pure-Rust 7z) · `zip` (fallback) · `clap`(derive) · `indicatif` · `anyhow` · `thiserror` · `toml` · `dirs`.

## Testing

- **Offline unit tests** against saved HTML fixtures (`vault_home.html`, `nes_list.html`, `armored_core_all.html`, `game_834.html`, `game_7818.html`). Snapshot parser outputs.
- **Synthetic 7z extraction test** (rom + junk) asserting junk is removed, ROM kept.
- `--features live` for manual integration tests against vimm.net (not in CI).

## Build/CI

- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` (offline).
- Release profile: `lto = true`, `codegen-units = 1`, `strip = true` → small static CLI binary.
- Cross-compile targets (rustls + no system deps): `x86_64-unknown-linux-musl`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`.

## Deferred (v2+)

- `vimm-bindings` UniFFI generation for iOS (Swift)/Android (Kotlin)/Python.
- Local metadata cache + download queue/resume.
- WASM web frontend.
- Interactive prompts (`dialoguer`).
- Mirror fallback (`dl.vimm.net` vs `dl3`) and headless-browser fallback if blocked.
- Checksum verification (MD5/SHA1) — hashes are for the original image, not the compressed format, so deferred until a conversion/verification strategy is decided.

## Open items (confirm at impl time, no user decision needed)

1. `dl3.vimm.net` POST: send `alt=0` explicitly vs. omit when primary format.
2. Verify dl3 always serves 7z and `sevenz-rust2` covers the codec (LZMA2/BCJ); keep zip fallback.
3. Confirm inner 7z entry filename is usable as the final ROM name.
4. Validate `.bin`+`.cue` keep-all on a real PS1/Sega CD download.

---

# Issue Plan

## Milestones

| # | Milestone | Covers |
|---|---|---|
| M1 | Foundation & Scaffolding | #1, #2 |
| M2 | Scraping the Vault | #3, #4, #5, #6 |
| M3 | Download Pipeline | #7, #8, #9 |
| M4 | CLI & Config | #10, #11 |
| M5 | Testing & Polish | #12, #13, #14 |

## Dependency graph

```
#1 ──► #2 ──► #3 ──┬──► #4 ──┐
                   ├──► #5 ──┼──► #10 ──► #11
                   └──► #6 ──┤      ▲
                        │    │      │
                        ▼    │     #12
                        #7 ──► #8 ──► #9
                         │      │
                         │      └──► #13
                         └─────────► #14 (final, depends on all)
```

## Issues

### M1 — Foundation & Scaffolding

**#1 — Scaffold Cargo workspace and crate skeletons**
- Theme: Cargo workspace & multi-crate layout
- Summary: Set up the vimm-downloader Cargo workspace (`vimm-core`, `vimm-cli`, `vimm-bindings`) + CI.
- Problem statement: Empty repo (README only). Need the portable-core + thin-frontend architecture from the locked design before any feature work, producing a static, cross-compilable CLI via rustls.
- Acceptance criteria:
  - [ ] Root `Cargo.toml` workspace with members `crates/vimm-core`, `crates/vimm-cli`, `crates/vimm-bindings`
  - [ ] `vimm-core`: `rlib + cdylib + staticlib`; `vimm-cli`: binary; `vimm-bindings`: compiling stub
  - [ ] Shared deps use `rustls-tls` (reqwest) — no OpenSSL anywhere
  - [ ] Release profile: `lto=true, codegen-units=1, strip=true`
  - [ ] `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` pass
  - [ ] GitHub Actions: fmt + clippy + test on push/PR
- Resources: Locked design → Workspace section. Related: none (root).

**#2 — Define core data model and error types**
- Theme: Rust type modeling & thiserror
- Summary: Implement `model.rs` (System, SearchQuery, GameSummary, GameDetail, Media, Format, ExtraFlag, Sort, Order, Op) + `error.rs`.
- Problem statement: All parsers and the CLI need a shared, well-typed model first. Library errors must be strongly typed (thiserror); CLI uses anyhow.
- Acceptance criteria:
  - [ ] `model.rs` defines all structs/enums with serde derives
  - [ ] `SearchQuery.system: Option<String>`; `mode` derived (None/"all" → adv, Some → list)
  - [ ] `Format { key, label, description, alt: u8, zipped_size_bytes }`
  - [ ] `GameSummary.extras: Vec<ExtraFlag>` (T/D/P/U/B) + `rating: Option<f32>`
  - [ ] `error.rs`: `VimmError` enum (Http, Parse, Io, Archive, Config)
  - [ ] Unit test for `SearchQuery` → mode derivation
- Resources: Locked design → Data model. Depends on: #1.

### M2 — Scraping the Vault

**#3 — Implement robust HTTP client**
- Theme: Async HTTP, reqwest, rustls, retry & rate limiting
- Summary: `client.rs` — `VimmClient` with browser UA, cookie store, retry/backoff, 500ms rate limit, timeouts.
- Problem statement: vimm.net is ad-funded with anti-bot/cookie behavior; we must be polite and resilient to avoid blocks and transient failures. rustls keeps the binary portable.
- Acceptance criteria:
  - [ ] `VimmClient::new()` / `with_config(Config)`
  - [ ] Browser UA, `cookie_store=true`, rustls, gzip, redirects
  - [ ] Retry w/ exp backoff on 5xx/network (3 attempts), configurable timeout
  - [ ] Min 500ms between requests (configurable rate limiter)
  - [ ] Public `get_text` / `post_stream` methods
  - [ ] Unit tests with `wiremock` for retry + rate-limit
- Resources: Locked design → Pragmatic robustness. Depends on: #1, #2.

**#4 — Systems parser + `systems` command**
- Theme: HTML scraping with scraper
- Summary: Parse 33 consoles from `/vault` (slug, name, launch year); expose `list_systems()` + `systems` subcommand.
- Problem statement: Users must discover non-obvious slugs (`X360-D`, `PSP`, `NES`) required by `search --system`. The `/vault` submenu + `title="Launched …"` spans provide this.
- Acceptance criteria:
  - [ ] `systems::parse(html) -> Vec<System>` from `#subMenu` links
  - [ ] `VimmClient::list_systems()` with in-memory cache
  - [ ] `vimm-downloader systems` table + `--json`
  - [ ] Fixture `vault_home.html` + snapshot test
- Resources: Research → `/vault` HTML. Depends on: #3, #10.

**#5 — Search parser (per-system + all-system dual schema)**
- Theme: Robust HTML table parsing & dual-schema handling
- Summary: Parse `/vault/?p=list&…` handling both schemas (per-system has Rating, all-system has System column), extras badges, decoy hrefs.
- Problem statement: All-system search (`mode=adv`, empty `system`) and per-system render different columns; all-system drops Rating. Rows contain decoy `/vault/999999` links + inline extras badges.
- Acceptance criteria:
  - [ ] `search::parse(html, query) -> Vec<GameSummary>` detects schema by first `<th>`
  - [ ] Skips decoy `999999`; extracts id, title, regions, version, languages
  - [ ] Parses extras badges `<b class="redBorder" title="Demo">D</b>` → `ExtraFlag`
  - [ ] `rating` only in per-system mode
  - [ ] `VimmClient::search(SearchQuery)` builds correct params (`mode=adv`+empty system when None/"all")
  - [ ] Fixtures `nes_list.html`, `armored_core_all.html` + snapshot tests
- Resources: Research → NES list + armored-core all-system pages. Depends on: #3, #2, #10.

**#6 — Detail parser (media JSON, base64 titles, version/disc/format selects)**
- Theme: Serde JSON + base64 + multi-dimensional selection
- Summary: Parse `/vault/{id}` — embedded `media` JSON, base64 `GoodTitle`, metadata table, `dl_version`/`disc_number`/`dl_format` selects → `Media`/`Format`.
- Problem statement: A game's file is selected across three dimensions (version, disc, format). The page embeds a `media` JS array (valid JSON) with base64 titles + `Mirror[]` format list; selects declare labels/descriptions and the site's `selected` default.
- Acceptance criteria:
  - [ ] `detail::parse(html) -> GameDetail` extracts `let media=[…]` via regex + serde_json
  - [ ] Decodes base64 `GoodTitle`
  - [ ] Builds `Media.formats` from `Mirror[]` + `Zipped/AltZipped/AltZipped2` by index; synthesizes single format when `#dl_format` absent
  - [ ] Parses `#dl_version` (incl. `selected`), `#disc_number`, `#dl_format` options (label+title)
  - [ ] Parses metadata table (region, players, year, publisher, serial, ratings, verified_date)
  - [ ] Fixtures `game_834.html` (single-format) + `game_7818.html` (3×3 multi-format) + snapshot tests
- Resources: Research → games 834 + 7818. Depends on: #3, #2.

### M3 — Download Pipeline

**#7 — Download streaming with progress**
- Theme: Streaming I/O & progress reporting
- Summary: `POST dl3.vimm.net` (`mediaId`, `alt`), stream to `.{stem}.7z.tmp` with indicatif progress.
- Problem statement: ROMs range KB → ~1GB; must stream to disk (not buffer), with live progress and resumable `.tmp` naming. `alt` selects the format variant.
- Acceptance criteria:
  - [ ] `VimmClient::download(media_id, alt, dest, progress_cb)` streams to `.7z.tmp`
  - [ ] Uses `Content-Length` for progress; indeterminate bar fallback
  - [ ] indicatif bar in CLI; progress callback hook in core (for bindings)
  - [ ] Atomic rename on success; cleanup `.tmp` on error
  - [ ] Honors rate limit + retry on the POST
- Resources: Locked design → Download pipeline. Depends on: #3, #6.

**#8 — Archive extraction + junk removal**
- Theme: 7z extraction & file-by-elimination logic
- Summary: Detect by magic bytes (7z primary, zip fallback), extract to `--out`, delete junk by blocklist, delete archive; `--archive` / `--keep-extras` modes.
- Problem statement: Downloads are 7z archives with ROM + junk (txt/nfo/diz/jpg/png/html/url). By elimination we keep the ROM (and companions like .cue/.gdi) without an extension allowlist.
- Acceptance criteria:
  - [ ] `archive::extract(path, out, opts)` detects format by magic bytes; `sevenz-rust2` (+ `zip` fallback)
  - [ ] Default: extract all, delete junk blocklist, delete archive
  - [ ] `--archive`: rename `.7z.tmp` → `.7z`, no extraction
  - [ ] `--keep-extras`: extract, keep junk
  - [ ] Final filename = 7z inner entry name (fallback: GoodTitle stem + format ext)
  - [ ] Unit test with synthetic 7z (rom.nes + readme.txt) — junk removed, rom kept
- Resources: Locked design → Extraction default. Depends on: #7.

**#9 — Spike — validate open download assumptions**
- Theme: Empirical validation / research spike
- Summary: Confirm dl3 POST shape, 7z codec coverage, inner filename sanity, multi-`.bin`+`.cue` keep-all on a real PS1/Sega CD download.
- Problem statement: Several pipeline assumptions can only be confirmed against the live site before finalizing request shape and extraction rules.
- Acceptance criteria:
  - [ ] Document `alt=0` send-explicit vs omit for primary format
  - [ ] Confirm dl3 always serves 7z + `sevenz-rust2` handles codec (LZMA2/BCJ); note any zip cases
  - [ ] Capture inner entry filename for a sample game; confirm usable
  - [ ] Download a multi-disc PS1/Sega CD title; confirm `.bin`+`.cue` (and multiple `.bin`) survive junk blocklist
  - [ ] Update code comments / design doc with findings
- Resources: Locked design → Open items. Depends on: #7, #8.

### M4 — CLI & Config

**#10 — clap command structure + JSON output**
- Theme: clap derive API & structured output
- Summary: Wire `vimm-cli` with `systems`, `search`, `info`, `download` subcommands + `--json` on each.
- Problem statement: The CLI is the v1 frontend; needs a consistent surface matching the design, with human tables by default and machine JSON for scripting.
- Acceptance criteria:
  - [ ] clap derive: `systems`, `search`, `info`, `download`
  - [ ] `search` flags: `--system` (optional), `--query`, `--region`, `--sort`, `--order`, `--limit`, `--players`, `--year`, `--publisher`, `--json`
  - [ ] `download` flags: `--version`, `--disc`, `--format`, `--out`, `--archive`, `--keep-extras`, `--config`, `-v`, `--base-url`
  - [ ] `--json` on every subcommand; default human-readable tables
  - [ ] Defaults: newest version / disc 1 / first `#dl_format` option
- Resources: Locked design → CLI surface. Depends on: #4, #5, #6, #7, #8.

**#11 — Config file with per-system format preferences**
- Theme: TOML config & layered defaults
- Summary: Load `~/.config/vimm-downloader/config.toml`; apply `[formats] System="key"` with order CLI flag > config > site default.
- Problem statement: Users want to persist a preferred format per system instead of passing `--format` each time; resolution must be deterministic.
- Acceptance criteria:
  - [ ] `Config::load()` reads config dir (overridable via `--config`)
  - [ ] Parses `[formats]` map (slug → format key)
  - [ ] `resolve_format(system, cli_flag, config)` implements CLI > config > site-default
  - [ ] Missing/malformed config = soft warning, not hard error
  - [ ] Unit tests for resolution order
- Resources: Locked design → Config. Depends on: #6, #10.

### M5 — Testing & Polish

**#12 — Offline HTML fixtures + parser snapshot tests**
- Theme: Snapshot/golden testing
- Summary: Save representative HTML fixtures + snapshot tests for systems/search/detail parsers; CI runs fully offline.
- Problem statement: CI must not hit vimm.net; parsers need regression protection as site HTML evolves.
- Acceptance criteria:
  - [ ] Fixtures in `tests/fixtures/`: `vault_home.html`, `nes_list.html`, `armored_core_all.html`, `game_834.html`, `game_7818.html`
  - [ ] Snapshot tests (insta or hand-rolled) for systems, both search schemas, single- + multi-format detail
  - [ ] All pass with `cargo test` (no network)
  - [ ] Document how to refresh fixtures
- Resources: Pages fetched during design. Depends on: #4, #5, #6.

**#13 — Synthetic 7z extraction test**
- Theme: Filesystem test fixtures
- Summary: Build a synthetic 7z (rom + junk) at test time; assert junk-by-elimination keeps ROM, removes junk.
- Problem statement: Extraction/junk-removal is the riskiest file logic and must be tested without real ROM downloads.
- Acceptance criteria:
  - [ ] Test builds 7z with `game.nes` + `readme.txt` + `cover.jpg`
  - [ ] Asserts: `game.nes` exists, junk deleted, archive deleted
  - [ ] `--keep-extras` path: junk retained
  - [ ] `--archive` path: 7z kept, no extraction
- Resources: Locked design → Extraction rule. Depends on: #8.

**#14 — Live integration flag + release profile + cross-compile**
- Theme: Feature flags & cross-compilation
- Summary: Gate network tests behind `--features live`, finalize release profile, document cross-compile targets.
- Problem statement: Live tests must stay out of CI but be available manually; CLI must ship as a small static binary on Linux/macOS/Windows.
- Acceptance criteria:
  - [ ] `#[cfg(feature = "live")]` integration tests (run with `cargo test --features live`)
  - [ ] Release profile confirmed: `lto=true, codegen-units=1, strip=true`
  - [ ] Documented cross-compile: `x86_64-unknown-linux-musl`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`
  - [ ] Binary size recorded in README/release notes
- Resources: Locked design → Build/CI. Depends on: all.
