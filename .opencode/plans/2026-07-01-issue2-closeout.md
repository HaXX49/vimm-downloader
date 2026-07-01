# Issue #2 Close-Out: rating f32 + Ratings re-export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply the two code changes identified by the issue #2 code review (Important I1 + Minor M1), verify the workspace is green, then close GitHub issue #2.

**Architecture:** No structural change. Two one-line edits to `vimm-core`: widen `SearchQuery.rating` from `Option<(Op, u32)>` to `Option<(Op, f32)>` (and drop the now-invalid `Eq` derive on `SearchQuery`), plus add `Ratings` to the crate-root re-export. Existing tests remain the verification gate; no new tests are added (M3 was explicitly scoped out by the user).

**Tech Stack:** Rust, thiserror, serde. CI gates: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.

## Global Constraints

- Rustls-only, `forbid(unsafe_code)` (already enforced in `crates/vimm-core/src/lib.rs:6`).
- No new dependencies.
- No new tests (user scoped to I1 + M1 only; M3 serde round-trip tests deferred).
- Commit message style: conventional commits, matching repo history (`feat:`, `fix:`, `docs:`, `refactor:`).
- Do not push or create a PR unless the user asks. Do not commit unless the user asks.

## Context (from code review)

Code review of issue #2 (commit `ddd1567`) found all 6 acceptance criteria **Met**, no Critical issues, one Important issue, and four Minor issues. The user chose to fix **I1** and **M1** only, then close #2.

- **I1 — `SearchQuery.rating` type mismatch.** `crates/vimm-core/src/model.rs:125` is `Option<(Op, u32)>`; DESIGN.md:39 specifies `Option<(Op, f32)>`. Vimm ratings are 0.0–10.0 floats; `u32` cannot represent `rating >= 8.5`, which would force rework in #5 (search parser) and #10 (CLI `--rating`). Because `f32` is not `Eq`, the `Eq` derive on `SearchQuery` (`model.rs:109`) must also be dropped (leave `PartialEq`). Nothing in the codebase relies on `SearchQuery: Eq` (tests use `assert_eq!` on `SearchMode`, which only needs `PartialEq`); `SearchQuery` does not derive `Hash`.
- **M1 — `Ratings` not re-exported.** `crates/vimm-core/src/lib.rs:15-18` re-exports 11 types but omits `Ratings` (the other helper, `SearchMode`, is present). Add `Ratings` for API symmetry; consumers (#6 detail parser, #10 CLI `--json`) will want `vimm_core::Ratings`.

---

### Task 1: Apply the I1 + M1 edits

**Files:**
- Modify: `crates/vimm-core/src/model.rs:109` (drop `Eq` on `SearchQuery`)
- Modify: `crates/vimm-core/src/model.rs:125` (`u32` → `f32`)
- Modify: `crates/vimm-core/src/lib.rs:15-18` (re-export `Ratings`)

**Interfaces:**
- Consumes: none (pure type edit).
- Produces: `vimm_core::Ratings` at crate root; `SearchQuery.rating: Option<(Op, f32)>`.

- [ ] **Step 1: Drop `Eq` from the `SearchQuery` derive**

In `crates/vimm-core/src/model.rs`, change line 109 from:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SearchQuery {
```

to:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SearchQuery {
```

(Match the two-line block exactly so the edit targets `SearchQuery` and not the other structs that share the derive line.)

- [ ] **Step 2: Widen `rating` to `f32`**

In `crates/vimm-core/src/model.rs`, change lines 123–125 from:

```rust
    /// Rating filter (only meaningful in per-system mode; the all-system
    /// results table has no Rating column).
    pub rating: Option<(Op, u32)>,
```

to:

```rust
    /// Rating filter (only meaningful in per-system mode; the all-system
    /// results table has no Rating column).
    pub rating: Option<(Op, f32)>,
```

- [ ] **Step 3: Re-export `Ratings` from the crate root**

In `crates/vimm-core/src/lib.rs`, change lines 15–18 from:

```rust
pub use model::{
    ExtraFlag, Format, GameDetail, GameSummary, Media, Op, Order, SearchMode, SearchQuery, Sort,
    System,
};
```

to:

```rust
pub use model::{
    ExtraFlag, Format, GameDetail, GameSummary, Media, Op, Order, Ratings, SearchMode, SearchQuery,
    Sort, System,
};
```

(`Ratings` is placed alphabetically between `Order` and `SearchMode`.)

- [ ] **Step 4: Sanity-check that nothing else references `SearchQuery: Eq` or `rating` as `u32`**

Run: `rg -n "SearchQuery" --type rust` and `rg -n "rating" --type rust`
Expected: only the model definition and the existing tests; no `HashMap<SearchQuery, _>` or `BTreeMap` keyed on `SearchQuery` (would need `Eq`+`Hash`/`Ord`). If any are found, stop and surface them.

- [ ] **Step 5: Commit (only if the user has asked to commit)**

```bash
git add crates/vimm-core/src/model.rs crates/vimm-core/src/lib.rs
git commit -m "fix(core): use f32 for SearchQuery rating and re-export Ratings"
```

Do not commit or push unless the user explicitly asks.

---

### Task 2: Verify the workspace is green

**Files:** none (read-only verification).

- [ ] **Step 1: Format check**

Run: `cargo fmt --check`
Expected: no output, exit 0.

- [ ] **Step 2: Clippy with warnings as errors**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: compiles clean, no warnings. Watch specifically for: `clippy::eq_op`/`derive_hash_xor_eq` is N/A (no `Hash` on `SearchQuery`); `missing_docs` still satisfied (no public item lost its docs).

- [ ] **Step 3: Run the offline test suite**

Run: `cargo test`
Expected: all tests pass, including the three `mode_*` tests and the three `*_renders_site_param` tests in `model.rs:269-315`. No test references `rating` as `u32`, so no test source changes are required.

- [ ] **Step 4: If any gate fails, stop and report**

If `cargo fmt`, `cargo clippy`, or `cargo test` fails, do not close #2. Surface the exact failure output and propose a fix before proceeding.

---

### Task 3: Close GitHub issue #2

**Files:** none (GitHub state change).

- [ ] **Step 1: Close the issue with a summary comment**

Run:

```bash
gh issue close 2 --comment "Closing #2. Code review confirmed all six acceptance criteria are met (model.rs + error.rs complete, serde-derived, documented; mode() derivation and unit tests in place). Applied two follow-up fixes from review: (1) \`SearchQuery.rating\` widened from \`Option<(Op, u32)>\` to \`Option<(Op, f32)>\` to match DESIGN.md and support fractional rating filters like \`>= 8.5\` (dropped \`Eq\` on \`SearchQuery\` since \`f32\` is not \`Eq\`); (2) \`Ratings\` added to the crate-root re-export. \`cargo fmt --check\`, \`cargo clippy -- -D warnings\`, and \`cargo test\` all green. Proceeding to #3 (HTTP client)."
```

Expected: issue #2 transitions to CLOSED; comment posted.

- [ ] **Step 2: Confirm**

Run: `gh issue view 2 --json state`
Expected: `"state":"CLOSED"`.

---

## Self-Review

- **Spec coverage:** I1 (rating f32 + drop Eq) → Task 1 steps 1–2. M1 (Ratings re-export) → Task 1 step 3. Verification → Task 2. Issue closure → Task 3. User explicitly scoped out M2/M3/M4. ✔
- **Placeholder scan:** No TBD/TODO; all code blocks are concrete. ✔
- **Type consistency:** `SearchQuery.rating` is `Option<(Op, f32)>` everywhere it appears in the plan; `Ratings` matches the struct name defined at `model.rs:255`. ✔
