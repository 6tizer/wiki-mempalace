# Audit v2 PR 05 QueryServed Privacy Handoff

## Summary

Branch: `codex/audit-v2-05-queryserved-privacy`

This PR implements Audit Report Follow-up v2 item 5:

- New `QueryServed` events store a stable salted hash instead of raw query
  text.
- New events include hash schema version, payload schema version, viewer scope,
  and optional redacted preview.
- `query_fingerprint` remains present for older readers, but new events fill it
  with the hash.
- Old `QueryServed` NDJSON remains readable through serde defaults.
- M12/suggest now checks event `viewer_scope` before using query-history events,
  while keeping legacy `top_doc_ids` fallback.
- New scoped events are skipped when a strategy scan has no explicit viewer.

## Files Changed

- `crates/wiki-core/Cargo.toml`
- `crates/wiki-core/src/events.rs`
- `crates/wiki-kernel/src/engine.rs`
- `crates/wiki-kernel/src/strategy.rs`
- `crates/wiki-cli/src/main.rs`
- `crates/wiki-cli/tests/query_privacy.rs`
- compatibility updates in storage/CLI/bridge tests
- `docs/outbox-event-matrix.md`
- `docs/specs/m12-strategy/`
- `docs/roadmap.md`
- `docs/specs/README.md`
- `docs/LESSONS.md`

## Verification

Focused checks:

- `cargo test -p wiki-core events -- --nocapture`
- `cargo test -p wiki-kernel query_served -- --nocapture`
- `cargo test -p wiki-kernel strategy -- --nocapture`
- `cargo test -p wiki-cli --test query_privacy -- --nocapture`
- `cargo test -p wiki-mempalace-bridge event_matrix_doc_stays_in_sync_with_wiki_event_variants -- --nocapture`

Focused review found no P0/P1/P2. Full required gates passed:

- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `git diff --check`
