# Tasks: Palace Init

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
| Init/report command shape | Agent | Subagent C + Main wiring | `crates/wiki-cli/` | B2 outbox shape | Done |
| Bridge consume validation | Agent | Subagent C + Main fixes | `crates/wiki-mempalace-bridge/`, tests | Init command | Done |
| Query/explain smoke | Agent | Main fixes | `crates/wiki-cli/tests/` | Palace init | Done |
| Handoff | Script | Subagent C | `docs/handovers/palace-init/summary.md` | Review | Done |

## Review Notes

- Focused review must prove rerun safety and `shared:wiki` bank use.

## Stop Conditions

- Stop if source full text becomes default palace input.
- Stop if consumer ack would hide unresolved required events.

## Verification

- focused palace init tests
- query/explain smoke tests
- `cargo fmt --all -- --check`
- `cargo test --workspace`
