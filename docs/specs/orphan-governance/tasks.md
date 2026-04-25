# Tasks: Orphan Governance

## Checklist

- [x] Requirements approved
- [x] Design approved
- [x] Plain architecture approved
- [x] Plan approved
- [x] Branch created
- [x] Tasks graded as Script / Skill / Agent
- [x] Subagent tasks assigned where needed
- [ ] Implementation complete
- [ ] Module review complete
- [ ] Module handoff written
- [ ] Tests added/updated
- [ ] Docs updated
- [ ] Integration review complete
- [ ] PR opened
- [ ] Codex/GitHub review addressed
- [ ] CI green
- [ ] Merged
- [ ] Roadmap/PRD updated

## Subtasks

| Task | Grade | Owner | Files | Depends on | Status |
| --- | --- | --- | --- | --- | --- |
| Review B1 orphan report | Skill / Agent | Main | B1 report | B1 complete | Deferred until fresh production audit |
| Draft cleanup policy | Agent | Main | `docs/specs/orphan-governance/` | B1 report review | Deferred |
| User approval gate | Skill | User/Main | cleanup plan | Draft policy | Deferred |
| Future implementation split | Agent | TBD | TBD after policy | User approval | Deferred |

## Review Notes

- B5 must not block B1-B4.

## Stop Conditions

- Stop before any vault mutation.
- Stop if B1 report is missing or stale.

## Verification

- B5 planning uses fresh B1 report.
- Future implementation must add dry-run and mutation tests.
