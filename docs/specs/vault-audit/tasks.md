# Tasks: Vault Audit

## Checklist

- [x] Requirements approved
- [x] Design approved
- [x] Plain architecture approved
- [x] Plan approved
- [x] Branch created
- [x] Tasks graded as Script / Skill / Agent
- [x] Subagent tasks assigned where needed
- [x] Implementation complete
- [x] Module review complete
- [x] Module handoff written
- [x] Tests added/updated
- [x] Docs updated
- [x] Integration review complete
- [x] PR opened
- [x] Codex/GitHub review addressed
- [x] CI green
- [x] Merged
- [x] Roadmap/PRD updated

## Subtasks

| Task | Grade | Owner | Files | Depends on | Status |
| --- | --- | --- | --- | --- | --- |
| Audit report model | Agent | Subagent A | `crates/wiki-cli/` | Spec approval | Done |
| Vault scanner | Agent | Subagent A | `crates/wiki-cli/` | Report model | Done |
| CLI command | Agent | Main | `crates/wiki-cli/src/main.rs` | Scanner | Done |
| Tests | Agent | Subagent A | `crates/wiki-cli/tests/` | CLI command | Done |
| Handoff | Script | Subagent A | `docs/handovers/vault-audit/summary.md` | Review | Done |

## Review Notes

- Focused review must prove no vault/DB mutation.

## Stop Conditions

- Stop if audit implementation needs to mutate files.
- Stop if orphan cleanup sneaks into B1.

## Verification

- `git diff --check`
- focused audit tests
- `cargo fmt --all -- --check`
- `cargo test --workspace`
