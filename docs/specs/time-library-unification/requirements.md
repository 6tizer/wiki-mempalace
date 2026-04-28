# Requirements: Time Library Unification

## Functional Requirements

- **R1 remove chrono**: Remove direct `chrono` dependencies from workspace crates.
- **R2 preserve format**: Keep generated timestamps as RFC3339 strings.
- **R3 live bridge**: Keep `wiki-mempalace-bridge/live` compiling without `chrono`.
- **R4 mempalace CLI**: Keep `rust-mempalace` CLI/service timestamp behavior equivalent.
- **R5 auditability**: Document that no separate `chrono` boundary remains.

## Acceptance Criteria

- [x] `rust-mempalace` uses `time` for current RFC3339 timestamps.
- [x] `wiki-mempalace-bridge/live` uses `time` for current RFC3339 timestamps.
- [x] `Cargo.toml` no longer declares `chrono`.
- [x] PR #80 + quick CI green.

## Checklist

- [x] No persisted schema change.
- [x] No CLI field rename.
- [x] No external timestamp format change.
