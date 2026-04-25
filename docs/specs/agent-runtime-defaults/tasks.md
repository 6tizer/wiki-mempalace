# Tasks: Agent Runtime Defaults

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
- [ ] PR opened
- [ ] Codex/GitHub review addressed
- [ ] CI green
- [ ] Merged
- [ ] Roadmap/PRD updated

## Subtasks

| Task | Grade | Owner | Files | Depends on | Status |
| --- | --- | --- | --- | --- | --- |
| Runtime command runbook | Script | Subagent D + Main fixes | `docs/`, `AGENTS.md` | B1-B3 command shape | Done |
| MCP scope/default review | Agent | Subagent D | `crates/wiki-cli/src/mcp.rs`, tests | Plan approval | Done |
| CLI default review | Agent | Main | `crates/wiki-cli/src/main.rs`, tests | Plan approval | Done |
| Handoff | Script | Subagent D | `docs/handovers/agent-runtime-defaults/summary.md` | Review | Done |

## Review Notes

- Focused review must prove docs do not point agents at repo-root `wiki.db`.

## Stop Conditions

- Stop before changing machine-global config.
- Stop if private scope support would be removed.

## Verification

- docs command scan
- default resolution tests if code changes
- `git diff --check`
