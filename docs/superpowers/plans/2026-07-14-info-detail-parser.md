# Current Detail-Page Parsing Implementation Plan

**Goal:** Restore complete `info` output for current Vimm detail pages while
preserving legacy parsing and public API compatibility.

## 1. Add representative offline fixtures

- Add sanitized current single-format and multi-format detail HTML fixtures.
- Include only the Open Graph title, active system navigation, metadata table,
  media JSON, selectors, and download form needed by the parser.
- Keep all test execution network-free.

## 2. Update media and detail parsing

- Extend raw media deserialization for `GoodDate` and normalize dates.
- Treat Vimm size values as KiB and map primary/alternative sizes by alt index,
  with compatibility for legacy array-shaped values.
- Enrich formats by selector order/value and normalize labels into stable keys.
- Add current title, system, region, metadata, split-rating, serial, and verified
  date extraction with legacy fallbacks.
- Add focused parser tests for both new fixtures and retain legacy tests.

## 3. Improve human CLI presentation

- Extract small formatting helpers for unavailable strings, years, ratings,
  and byte sizes.
- Print `N/A` for genuinely unavailable detail values.
- Print corrected sizes with KB/MB/GB units without altering JSON output.
- Add unit tests for formatting boundaries and missing values.

## 4. Document and verify

- Add issue #27 findings and checked acceptance criteria to `DESIGN.md`.
- Add detail snapshots for current fixtures and update legacy snapshots only
  where corrected byte units intentionally change output.
- Run `cargo test` and `cargo clippy -- -D warnings`.
- Commit code/tests and documentation as logical changes, then push the branch
  and open a PR after verification.
