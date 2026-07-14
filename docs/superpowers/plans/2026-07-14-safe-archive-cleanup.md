# Safe Archive Cleanup Implementation Plan

**Goal:** Ensure extraction cleanup can only affect files created by the current
download and never overwrite pre-existing output files.

## 1. Add output-local staging support

- Promote `tempfile` to a workspace dependency used by `vimm-core`.
- Create a guarded temporary extraction directory inside `out_dir`.
- Extract and apply junk filtering exclusively inside staging.

## 2. Publish files safely

- Map staged files to output destinations using archive-relative paths.
- Preflight every destination and reject existing files/type conflicts before
  moving any staged file.
- Create required parent directories and rename retained files into place.
- Remove the internal archive only after successful publication unless
  `keep_archive` is enabled.
- Add operation and path context to filesystem errors without changing public
  error variants.

## 3. Add regression coverage

- Retain existing ZIP/7z, junk, extras, and archive-option tests.
- Prove unrelated top-level and nested junk-extension files remain unchanged.
- Prove collisions preserve existing files, publish nothing, retain the archive,
  and clean staging.
- Prove nested archive paths are preserved and success leaves no staging tree.

## 4. Verify and publish

- Record issue #29 findings and checked criteria in `DESIGN.md`.
- Run `cargo fmt --all -- --check`, `cargo test`, and
  `cargo clippy -- --deny warnings`.
- Commit the implementation, push `issue-29/safe-archive-cleanup`, and open a
  draft PR linked to issue #29.
