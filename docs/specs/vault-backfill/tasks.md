# Tasks: Vault Backfill

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
- [ ] Codex/GitHub review addressed
- [x] CI green
- [ ] Merged
- [ ] Roadmap/PRD updated

## Subtasks

| Task | Grade | Owner | Files | Depends on | Status |
| --- | --- | --- | --- | --- | --- |
| Stable ID planner | Agent | Subagent B | `crates/wiki-cli/` | B1 report shape | Done |
| Frontmatter ID writer | Agent | Subagent B | `crates/wiki-cli/` | Planner | Done |
| DB backfill path | Agent | Subagent B + Main fixes | `crates/wiki-cli/` | Planner | Done |
| Outbox behavior | Agent | Subagent B + Main fixes | `crates/wiki-cli/tests/` | DB backfill | Done |
| Tests | Agent | Subagent B + Main fixes | `crates/wiki-cli/tests/` | CLI path | Done |
| Handoff | Script | Subagent B | `docs/handovers/vault-backfill/summary.md` | Review | Done |

## Review Notes

- Focused review must prove idempotency and no duplicate logical records.

## Stop Conditions

- Stop before full-vault apply unless dry-run report exists.
- Stop if implementation would infer complex claims from Markdown.

## Verification

- focused backfill tests
- dry-run fixture test
- repeated-run fixture test
- `cargo fmt --all -- --check`
- `cargo test --workspace`
