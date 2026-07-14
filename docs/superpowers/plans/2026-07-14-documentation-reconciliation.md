# Documentation Reconciliation Implementation Plan

**Goal:** Make README and DESIGN accurately describe the completed post-merge
v1 without advertising planned or ineffective behavior.

## 1. Capture authoritative interfaces

- Record top-level and subcommand help from the current binary.
- Inspect release workflow artifact names and latest release metadata.
- Cross-check runtime modules, config behavior, extraction safety, fixtures, and
  normal/live test commands against code and CI.

## 2. Rewrite README

- Replace stale milestone status with current capabilities and support notes.
- Add release and source installation instructions plus executable quick starts.
- Document only effective CLI arguments, JSON, configuration, download options,
  safe staging, collision behavior, and platform invocation details.
- Refresh architecture, dependencies, development commands, legal note, and
  project links.

## 3. Reconcile DESIGN

- Update detail scraping, size normalization, and safe extraction data flow.
- Correct the CLI surface and offline/live validation guidance.
- Refresh fixture coverage, milestone status, and evidence-backed acceptance
  checkboxes while preserving historical spike and issue rationale.
- Add issue #31 findings and completion criteria.

## 4. Validate and publish

- Compare documentation against all captured help output and release assets.
- Check relative links and search for stale planned/coming language.
- Run formatting, offline tests, and clippy with warnings denied.
- Commit, push `issue-31/reconcile-documentation`, and open a draft PR linked to
  issue #31.
