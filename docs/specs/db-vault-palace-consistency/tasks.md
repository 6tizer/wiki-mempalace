# Tasks: DB/Vault/Palace Consistency Governance

| Task | Grade | Owner | Files / Area | Status |
| --- | --- | --- | --- | --- |
| PRD/spec trio | Script | Main | `docs/prd`, `docs/specs/db-vault-palace-consistency` | Done |
| Consistency audit | Agent | Subagent A | `crates/wiki-cli/src/consistency.rs`, audit tests | Done |
| Consistency plan | Agent | Main | plan schema/rendering/tests | Done |
| DB/Vault source-link fixes | Agent | Subagent C | DB-backed link candidates/projection tests | Done |
| Apply + palace replay integration | Agent | Main | apply path, CLI wiring, palace reuse | Done |
| Focused review | Agent | Reviewer | diff vs spec | Pending |
| Integration review + CI | Skill/Script | Main | workspace | Pending |
| PR + merge follow-up | Skill | Main | roadmap, LESSONS, handoff | Pending |

## Checklist

- [x] PRD records DB as canonical source.
- [x] Audit compares DB, Vault, and optional Mempalace.
- [x] Audit reports are timestamped JSON/Markdown siblings.
- [x] Markdown reports are Chinese.
- [x] Plan is generated only from audit evidence.
- [x] Plan rejects unknown paths and action types.
- [x] Dry-run writes nothing.
- [x] Apply mutates DB before Vault projection.
- [x] Apply repairs Mempalace page mirrors through replay/sink code.
- [x] Apply never direct-writes `palace.db`.
- [x] Source bodies are not inserted into Mempalace.
- [x] `batch-ingest` is never run by this feature.
- [ ] Focused review and integration review are complete.
- [ ] CI passes locally.
