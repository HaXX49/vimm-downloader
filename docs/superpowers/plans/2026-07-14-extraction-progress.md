# Extraction progress implementation plan

1. Refactor CLI extraction into an async helper that owns the archive and output
   paths and executes `vimm_core::extract` through `tokio::task::spawn_blocking`.
2. Finish the download progress bar as soon as network transfer completes, then
   show a distinct elapsed-time extraction spinner for human output.
3. Ensure success, extraction errors, and join errors terminate the spinner
   cleanly while JSON mode remains progress-free.
4. Add offline CLI tests for successful blocking extraction and failure
   propagation using synthetic ZIP archives.
5. Run formatting, offline tests, clippy with warnings denied, and focused CLI
   behavior checks.
6. Commit and publish issue #35 through a reviewed PR, then prepare, tag, and
   verify patch release v1.0.3 with all three platform assets.
