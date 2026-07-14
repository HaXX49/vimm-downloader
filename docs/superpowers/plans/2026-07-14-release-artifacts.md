# Issue #25 — Cross-Platform Release Artifacts Implementation Plan

> Branch: `issue-25/release-artifacts`. Spec:
> `docs/superpowers/specs/2026-07-14-release-artifacts-design.md`.

## Tasks

1. Change the release matrix to Linux x86-64 musl, macOS ARM64, and Windows
   x86-64 MSVC only.
2. Copy each compiled CLI to its unique release filename and upload the renamed
   artifact.
3. Make the release job create `v1.0.1` when absent and upload all three assets
   with `--clobber` for safe reruns.
4. Bump the workspace and lockfile package versions to `1.0.1`.
5. Run formatting, clippy, and offline tests; commit the implementation.
6. Push the branch, open and merge a PR after CI passes, then delete the branch.
7. Tag merged `main` as `v1.0.1`, monitor the tag workflow, and verify exactly
   three non-empty release assets.
