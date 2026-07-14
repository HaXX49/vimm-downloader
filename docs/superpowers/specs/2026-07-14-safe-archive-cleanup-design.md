# Safe Archive Cleanup Design

## Problem

`archive::extract` currently extracts directly into the requested output
directory and then recursively deletes every file whose extension matches the
junk blocklist. Since the CLI defaults `--out` to `.`, running from a populated
directory can delete unrelated user files. A protected unrelated file also
aborts cleanup with an unhelpful `Access is denied` error after the ROM has
already been extracted.

## Design

### Isolated staging

Create a uniquely named temporary directory inside `out_dir` and extract the
archive there. Placing staging inside the output directory keeps publication on
the same filesystem so retained files can be renamed without cross-device
failures. Junk deletion and empty-directory cleanup will operate only on this
staging tree.

`tempfile`, already used by tests, will become a workspace/runtime dependency
of `vimm-core`. Its `TempDir` guard removes staging contents automatically on
success and error.

### Publishing retained files

After optional junk cleanup, collect staging files and preserve their relative
archive paths when mapping them into `out_dir`. Before moving any file, preflight
every destination:

- existing directories may be reused;
- an existing destination file is never overwritten;
- any file/directory type conflict returns an explicit error before publication.

Once preflight succeeds, create required parent directories and rename each
retained file from staging into the output tree. Returned paths identify only
files published by the current extraction, never all files already in
`out_dir`.

Multi-file publication cannot be fully atomic, but collision errors occur before
the first move. Unexpected mid-publication I/O failures report the operation and
path; already published files remain visible and the temporary guard removes
only unpublished staging data.

### Archive lifecycle and errors

Remove the downloaded internal archive only after extraction, cleanup, and file
publication all succeed. Preserve it when `keep_archive` is enabled or any
earlier step fails, allowing recovery without another download.

Keep the public `VimmError` shape unchanged. Wrap relevant filesystem errors in
`std::io::Error` messages that identify the failed operation and path so Windows
create, extract, cleanup, collision, publish, and archive-removal failures are
distinguishable through the existing `VimmError::Io` variant.

## Testing

- ZIP and 7z default extraction removes archive-provided junk but retains the
  ROM and deletes the internal archive.
- `keep_extras` publishes archive-provided junk; `keep_archive` retains the
  downloaded archive.
- Pre-existing `.txt`, `.jpg`, and nested junk files in `out_dir` remain byte-for-
  byte unchanged.
- A pre-existing destination ROM causes a non-destructive collision error before
  any other archive entry is published and leaves the archive recoverable.
- Nested retained archive paths are preserved and staging directories are gone
  after both success and failure.
- Run `cargo test`, `cargo clippy -- -D warnings`, and formatting checks on
  Windows with all automated tests offline.

## Compatibility and Scope

The public `extract` signature, CLI arguments, junk blocklist, and default output
directory remain unchanged. This fix changes cleanup scope and collision safety
only; resumable downloads and user-selectable overwrite policies remain outside
issue #29.
