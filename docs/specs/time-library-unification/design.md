# Design: Time Library Unification

## Summary

The workspace already uses `time::OffsetDateTime` in wiki-core, wiki-kernel,
wiki-storage, wiki-cli, and most bridge paths. The only remaining `chrono`
usage was `Utc::now().to_rfc3339()` in `rust-mempalace` and the live
Mempalace bridge.

## Implementation

- Replace `chrono::Utc::now().to_rfc3339()` with helpers built on:
  `time::OffsetDateTime::now_utc().format(&Rfc3339)`.
- Add `time` as the direct dependency for `rust-mempalace`.
- Make bridge live feature depend on optional `time` instead of optional `chrono`.
- Keep RFC3339 string output unchanged at API and DB boundaries.

## Boundary Decision

No `chrono` boundary is retained. `rust-mempalace` does not need `chrono`
independence because its timestamp usage is simple current-time formatting and
does not expose chrono types.

## Test Strategy

- `cargo test -p rust-mempalace -p wiki-mempalace-bridge --features wiki-mempalace-bridge/live`;
- `cargo test --workspace`;
- `cargo clippy --workspace --all-targets -- -D warnings`;
- `cargo fmt --all -- --check`;
- `git diff --check`.
