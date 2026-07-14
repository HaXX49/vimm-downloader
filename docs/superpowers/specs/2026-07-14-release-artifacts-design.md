# Issue #25 — Cross-Platform Release Artifacts

> Status: **Awaiting approval**. Branch: `issue-25/release-artifacts`.

## Problem

The `v1.0.0` release workflow successfully builds multiple targets, but uploads
raw binaries with colliding basenames. The release therefore contains only one
generic `vimm-downloader` asset instead of distinct downloads for each platform.
The workflow also builds an Intel macOS target that is outside the requested
release set and deletes an existing release when rerun.

## Goals

- Publish exactly three executables: Linux x86-64 musl, macOS ARM64, and Windows
  x86-64 MSVC.
- Give every executable a unique, platform-specific release filename.
- Make release publication safe to rerun without deleting release metadata.
- Publish the correction as version and tag `v1.0.1`, preserving `v1.0.0`.

## Design

The release build matrix will contain these target and asset-name pairs:

| Rust target | Release asset |
| --- | --- |
| `x86_64-unknown-linux-musl` | `vimm-downloader-linux-amd64` |
| `aarch64-apple-darwin` | `vimm-downloader-macos-arm64` |
| `x86_64-pc-windows-msvc` | `vimm-downloader-windows-amd64.exe` |

After compiling `vimm-cli`, each matrix job will copy its binary to the matching
asset name and upload that uniquely named file. The release job will download
the artifacts into one directory, create the GitHub release only when it does
not already exist, and upload the three files with replacement enabled. This
makes a failed or manually rerun release job idempotent without deleting its
release notes or metadata.

The workspace version and local workspace-package entries in `Cargo.lock` will
be updated to `1.0.1`. No Rust API or runtime behavior changes.

## Release Process

After the implementation PR passes CI and is merged into `main`, create and push
the annotated `v1.0.1` tag. The tag-triggered workflow must complete before the
release is considered successful. The existing `v1.0.0` tag and release remain
unchanged.

## Verification

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features --no-fail-fast`
- Pull-request CI succeeds on Linux, macOS, and Windows.
- Release workflow succeeds for all three build jobs and the release job.
- Release `v1.0.1` is public, is not a prerelease, and contains exactly the three
  expected non-empty assets.

## Out of Scope

- Intel macOS binaries
- Archives, installers, checksums, or code signing
- Changes to the incomplete `v1.0.0` release
