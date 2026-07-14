# Extraction progress design

## Context

Issue #35 tracks a misleading pause after large downloads. The download bar
reaches 100%, but the CLI then calls the synchronous archive extractor and does
not update the terminal until extraction finishes. Star Fox: Assault for
GameCube advertises an archive near 976 MB, so decompression and Windows
filesystem scanning can make this silent phase last long enough to look hung.

The extraction implementation is intentionally synchronous because the ZIP and
7z libraries expose blocking APIs. Calling it directly from async `main` also
blocks Tokio's worker thread.

## Goals

- Make network transfer and extraction visibly separate phases.
- Keep Tokio's async worker available during blocking decompression.
- Preserve archive staging, collision checks, cleanup, and error semantics.
- Preserve JSON stdout as one final machine-readable document.
- Publish the fix as patch release v1.0.3.

## Non-goals

- Byte-accurate extraction progress; the archive libraries do not expose one
  consistently.
- Cancellation, resumable downloads, or parallel extraction.
- Changes to archive selection, output naming, or collision behavior.
- Downloading copyrighted game data as part of testing.

## Considered approaches

### 1. Print a one-time extraction message

Finish the download bar and print `Extracting...` before the existing call.
This is simple, but the process can still look inactive during a long extraction
and the blocking call remains on Tokio's worker.

### 2. Separate progress phases and use `spawn_blocking` (recommended)

Finish the network bar immediately after `download_rom` succeeds. For human
output, start an indeterminate spinner with elapsed time and an `Extracting`
message. Move the existing `vimm_core::extract` call into
`tokio::task::spawn_blocking`, passing owned paths and copyable extraction
options. On success, finish the spinner with the file count. On extraction or
join failure, abandon it with an error message before propagating the error.

This accurately communicates what the program is doing without changing the
core extraction API or claiming byte-level progress.

### 3. Add extraction callbacks to the core

Thread entry-level progress callbacks through both archive backends. This would
expand the public API and still could not provide useful byte progress for all
formats. It is unnecessary for resolving the reported bug.

## Detailed behavior

### Human output

1. Create and update the existing download progress bar.
2. After the archive has been flushed and renamed, finish it with
   `Downloaded; extracting...` when extraction is enabled.
3. Start a spinner using indicatif's steady tick and elapsed-time template.
4. Run extraction in `spawn_blocking`.
5. Finish the spinner with `Extracted N files to <out>`.
6. If extraction fails or its blocking task cannot be joined, abandon the
   spinner with `Extraction failed` and return the contextual error.

With `--archive`, retain the current archive-saved completion message and do not
create an extraction spinner.

### JSON output

Do not create progress indicators or write phase messages to stdout. Continue
emitting exactly one final JSON document. Errors remain on the CLI error path.

### Error handling and ownership

Clone the downloaded archive path and output path into the blocking closure.
Map a Tokio join failure into an `anyhow` error with extraction context. Return
the original `VimmError` from the closure unchanged so existing actionable I/O
and archive errors remain visible.

## Testing

- Extract CLI phase execution into a small async helper if needed to make the
  blocking boundary testable without network access.
- Add an offline synthetic ZIP test through the helper, asserting returned
  files and archive cleanup.
- Retain all core archive regression tests, especially non-destructive output
  and collision preservation.
- Run `cargo fmt --all -- --check`, `cargo test`, and
  `cargo clippy --all-targets --all-features -- -D warnings`.
- Confirm `--archive` and JSON branches remain free of extraction spinners by
  code-path tests or focused unit tests where practical.

## Release

After CI passes and the issue PR is merged, prepare v1.0.3 in a separate release
issue and branch, verify `vimm-downloader --version`, merge through CI, tag
`v1.0.3`, and confirm Windows, Linux, and macOS assets are uploaded.
