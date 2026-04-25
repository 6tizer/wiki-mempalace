# Tasks: Vault Report Paths

## Checklist

- [x] Requirements approved
- [x] Design approved
- [x] Branch created
- [x] Tasks graded as Script / Skill / Agent
- [x] Implementation complete
- [x] Focused review complete
- [x] Tests added/updated
- [x] Docs updated
- [x] Verification complete
- [x] Handoff written
- [ ] PR opened

## Subtasks

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| Spec trio | Script | Main | `docs/specs/vault-report-paths/` | Complete |
| CLI path helpers | Script | Main | `crates/wiki-cli/src/main.rs` | Complete |
| CLI command wiring | Script | Main | `crates/wiki-cli/src/main.rs` | Complete |
| Tests | Script | Main | `crates/wiki-cli/tests/` | Complete |
| Agent docs | Script | Main | `AGENTS.md`, spec index | Complete |
| Handoff | Script | Main | `docs/handovers/vault-report-paths/summary.md` | Complete |

## Verification

- `cargo test -p wiki-cli --test dashboard --quiet`
- `cargo test -p wiki-cli --test suggest --quiet`
- `cargo test -p wiki-cli --test metrics --quiet`
- `cargo test -p wiki-cli --test automation_run_daily --quiet`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`

All verification commands passed locally on branch `codex/vault-report-paths`.
