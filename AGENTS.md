# AGENTS.md — Rules of Work for vimm-downloader

## Project Overview

A portable CLI downloader for Vimm's Lair Vault (retro game ROM archive). Pure async Rust workspace with three crates: `vimm-core` (library), `vimm-cli` (binary), `vimm-spike` (live validation), `vimm-bindings` (UniFFI stub).

## Workflow Rules

### Before Any Implementation
1. **Always run brainstorming skill** before creating features, building components, or modifying behavior
2. **Write a spec** to `docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md` and commit it
3. **Get user approval** on the spec before writing implementation code
4. **Invoke writing-plans** after spec approval to create an implementation plan

### Code Standards
- `#![forbid(unsafe_code)]`, `#![warn(missing_docs)]`, pedantic clippy lints enforced
- No `reqwest` types leak into the public API (important for UniFFI)
- Every public item must have doc comments
- Follow existing patterns: error types via `thiserror`, serde derives on data models, wiremock for HTTP tests
- Rust only — no OpenSSL, rustls only for portability

### Testing
- Run `cargo test` and `cargo clippy -- -D warnings` before committing
- All tests must pass offline (no network required for `cargo test`)
- Live tests gated behind `--features live` (not in CI)
- Use offline HTML fixtures in `tests/fixtures/` for parser tests

### Git Workflow
- Work on each issue in a dedicated branch named `issue-<N>/<short-description>` (e.g. `issue-8/archive-extraction`)
- Branch off `main`, merge back via PR after review
- One commit per logical change with descriptive message
- Push to `origin/main` after each completed issue
- Delete feature branches after merge
- Never commit secrets, API keys, or credentials

### Issue Tracking
- Follow the issue plan in `DESIGN.md`
- Mark acceptance criteria as `[x]` when complete
- Update `DESIGN.md` open items section with findings after spikes

### Dependencies
- Check `Cargo.toml` workspace dependencies before adding new crates
- Prefer existing workspace deps over adding new ones
- `time = "=0.3.51"` is pinned — do not change without verifying cookie crate compatibility

### Communication
- Be concise — answer directly without preamble
- Show code references as `file_path:line_number`
- Run verification commands and show output before claiming success
