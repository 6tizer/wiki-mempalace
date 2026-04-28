# PRD: Time Library Unification

## Goal

Use one primary time library across the workspace so timestamp formatting and
dependency policy stay simple.

## Scope

- Replace remaining `chrono` usage with `time` where behavior is equivalent.
- Remove direct `chrono` dependencies from local crates.
- Keep timestamp wire format as RFC3339 strings.
- Document any boundary if a crate must keep `chrono`.

## Non-Goals

- Changing timestamp columns or persisted formats.
- Rewriting time math unrelated to `chrono`.
- Changing external CLI output fields.

## Success Criteria

- `rg "chrono" crates Cargo.toml` has no active crate dependency or source usage.
- `rust-mempalace` and live bridge tests pass.
- Workspace tests and clippy remain green.

## Status

- **Complete in PR #80** — replaced remaining `chrono::Utc` timestamps with `time::OffsetDateTime`; quick CI passed.
