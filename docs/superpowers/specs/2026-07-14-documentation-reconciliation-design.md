# Documentation Reconciliation Design

## Problem

The top-level documentation reflects an early project stage rather than the
completed post-merge v1. README marks download, archive extraction, config, CLI
wiring, and testing as planned even though they are implemented. Its quick-start
commands are commented placeholders, its architecture omits active modules and
`vimm-spike`, and its CLI reference advertises filters that the binary does not
accept or apply. The suggested `cargo test --all-features` command also enables
the network-gated live feature while describing the run as offline.

DESIGN retains useful architectural and historical rationale, but its detail
scraping and extraction pipeline sections predate issues #27 and #29. Several
early issue criteria remain unchecked despite the corresponding closed work.

## README structure

Replace the stale milestone-led introduction with a user-oriented document:

1. concise purpose, supported platforms, and current v1 capabilities;
2. installation from GitHub release artifacts plus a source-build alternative;
3. executable quick-start examples for systems, all-system/per-system search,
   info, and safe download output;
4. an accurate command reference derived from the binary's help output;
5. JSON and TOML format-preference examples;
6. download/extraction behavior, archive options, safe staging, and
   non-overwriting collision behavior;
7. current architecture/modules and development verification commands;
8. live-test opt-in, legal responsibility note, links to DESIGN and license.

Use platform-neutral command names in the main examples and add short
PowerShell-specific invocation notes rather than duplicating the full guide.
Release instructions will name the repository's actual platform assets. Source
build instructions will preserve the Rust 1.75 minimum and rustls-only stance.

Only implemented behavior will be documented. In particular, positional search,
query-less browsing, players/year/publisher filters, functional verbosity, and
resumable downloads remain excluded until their own implementations land.

## DESIGN reconciliation

Preserve DESIGN as the architectural record while updating factual sections:

- current detail metadata sources, `GoodDate`, numeric format alternatives, and
  KiB-to-byte normalization;
- output-local extraction staging, staging-only junk cleanup, destination
  collision refusal, and archive lifecycle;
- actual CLI surface and the distinction between implemented options and
  deferred improvements;
- current offline fixtures and validation commands;
- milestone completion and acceptance checkboxes for closed issues where the
  implementation and tests provide evidence.

Do not remove spike findings or issue history. Later issue entries (#27, #29,
#31) remain additive records rather than being folded into the original issue
descriptions.

## Validation

- Capture top-level and subcommand `--help` output and compare every documented
  flag, default, and positional argument.
- Run non-network examples for `--help`, `--version`, and parser-backed commands
  where fixtures or local execution make that deterministic; do not perform new
  ROM downloads for documentation validation.
- Confirm referenced release asset names against the release workflow and latest
  published release metadata.
- Check relative Markdown links and inspect the final diff for stale "planned",
  "coming", or contradictory offline/live wording.
- Run `cargo fmt --all -- --check`, `cargo test`, and
  `cargo clippy -- --deny warnings`.

## Scope and Compatibility

This issue changes README and DESIGN documentation only, plus the required
design/plan records. It does not alter Rust APIs, CLI parsing, release workflows,
or runtime behavior. Documentation targets post-merge `main` containing PRs #28
and #30; it does not describe the older v1.0.1 binary as if those fixes were
already released.
