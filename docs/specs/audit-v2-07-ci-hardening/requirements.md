# Requirements: Audit v2 PR 07 CI Hardening

## Goal

Make the required PR gate cover clippy and supply-chain policy checks while
keeping heavier audit jobs on scheduled/manual lanes.

## Functional Requirements

- `CI Quick` must run `cargo clippy --workspace --all-targets -- -D warnings`
  on pull requests and pushes to `main`.
- `CI Quick` must run `cargo-deny` on pull requests and pushes to `main`.
- `cargo-deny` must check:
  - advisories
  - yanked crates
  - allowed licenses
  - duplicate dependency versions
  - unknown registries and git sources
- Existing scheduled/manual `cargo audit` workflow remains scheduled/manual and
  must not become a required PR job in this PR.
- CI docs and PR templates must list the required local/PR gate consistently.

## Non-Goals

- Do not change branch-protection settings through the GitHub API.
- Do not remediate dependency version duplicates in this PR.
- Do not add Rust crate dependencies.
- Do not run production wiki or palace writes.

## Acceptance

- `cargo deny --all-features check advisories bans licenses sources` passes.
- `cargo fmt --all -- --check` passes.
- `cargo test --workspace` passes.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- Shell workflow/script syntax checks pass where applicable.
