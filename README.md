# vimm-downloader

[![CI](https://github.com/HaXX49/vimm-downloader/actions/workflows/ci.yml/badge.svg)](https://github.com/HaXX49/vimm-downloader/actions/workflows/ci.yml)

A portable command-line client for browsing game metadata and downloading files
from the [Vimm's Lair Vault](https://vimm.net/vault). It uses an async Rust core,
rustls instead of OpenSSL, and ships for Windows, Linux, and macOS.

> Use this tool only for content you are legally entitled to download. You are
> responsible for complying with the laws that apply to you and Vimm's Lair's
> terms.

## Install

Download the appropriate executable from
[GitHub Releases](https://github.com/HaXX49/vimm-downloader/releases):

| Platform | Release asset |
|---|---|
| Windows x86-64 | `vimm-downloader-windows-amd64.exe` |
| Linux x86-64 (musl) | `vimm-downloader-linux-amd64` |
| macOS Apple silicon | `vimm-downloader-macos-arm64` |

On Linux or macOS, make the downloaded file executable. The examples below use
`vimm-downloader`; in PowerShell, use the downloaded filename prefixed with
`./`, for example `./vimm-downloader-windows-amd64.exe systems`.

To build from source, install a current stable Rust toolchain and run:

```console
cargo build --release -p vimm-cli
```

The executable is written to `target/release/vimm-downloader` (or
`vimm-downloader.exe` on Windows).

## Quick start

```console
# Discover supported system slugs
vimm-downloader systems

# Search every system
vimm-downloader search --query "pokemon"

# Restrict a search to one system
vimm-downloader search --query "pokemon emerald" --system GBA

# Inspect a result using its ID
vimm-downloader info 5625

# Download and extract into a dedicated directory
vimm-downloader download 5625 --out ./roms
```

Using a dedicated `--out` directory keeps downloaded files easy to identify.
Extraction never overwrites an existing destination: a collision returns an
error and preserves the downloaded archive for recovery.

## Command reference

```text
vimm-downloader [--json] systems
vimm-downloader [--json] search --query <QUERY>
    [--system <SYSTEM>] [--sort <Title|Players|Year|Rating>]
    [--order <ASC|DESC>] [--limit <LIMIT>]
vimm-downloader [--json] info <ID>
vimm-downloader [--json] download <ID>
    [--format <FORMAT>] [--out <DIR>] [--archive]
    [--keep-extras] [--config <PATH>]
```

`--query` accepts a title substring of at least three characters. Omitting
`--system` searches across every system. `--json` provides machine-readable
output and may also be written after a subcommand. Run `vimm-downloader --help`
or `<command> --help` for clap's complete generated help.

Some reserved flags currently shown by generated help are not yet wired into
behavior. The reference above lists the effective v1 options.

## Downloads and formats

By default, `download` selects the site's first format, streams an archive into
`--out`, detects ZIP or 7z by its magic bytes, and extracts it. Archive entries
are first processed in a temporary staging directory inside `--out`; unrelated
files are never scanned or deleted. Files with common extra-material extensions
(`.txt`, `.nfo`, `.diz`, images, HTML, and URL files) are removed only from that
staging directory.

- `--archive` keeps the raw archive and skips extraction.
- `--keep-extras` retains extra material during extraction.
- `--format <key>` selects a format advertised by the game's detail page.
- `--out <dir>` chooses the destination; it defaults to the current directory.

Format preferences can be stored in TOML:

```toml
[formats]
GameCube = "rvz"
Wii = "ciso"
```

The default path is `~/.config/vimm-downloader/config.toml`; use `--config` to
choose another file. Resolution order is command-line `--format`, then the
per-system config value, then the site's first option.

## Architecture

The workspace separates network and filesystem behavior from presentation:

- `vimm-core`: models, HTTP client, parsers, download streaming, archive
  extraction, and configuration.
- `vimm-cli`: the `vimm-downloader` clap/indicatif frontend.
- `vimm-spike`: manually run live validation utilities.
- `vimm-bindings`: a compiling UniFFI facade stub for future frontends.
- `tests/fixtures`: offline HTML fixtures used by parser regression tests.

No `reqwest` type is exposed by the core public API. See
[`DESIGN.md`](./DESIGN.md) for the detailed architecture, empirical findings,
and issue history.

## Development

```console
cargo fmt --all -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

The normal test suite is offline. Live integration checks are opt-in and may
contact Vimm's Lair:

```console
cargo test --features live
```

## License

MIT
