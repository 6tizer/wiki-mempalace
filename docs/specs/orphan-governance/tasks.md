# Tasks: B5 Orphan Governance Follow-up

| Task | Grade | Owner | Files / Area | Status |
| --- | --- | --- | --- | --- |
| Spec follow-up | Script | Main | `docs/specs/orphan-governance/*` | Done |
| Audit scope + timestamp + path lists | Agent | Subagent A | `crates/wiki-cli/src/vault_audit.rs`, audit tests | Done |
| Root `concepts/` write-back bug | Agent | Subagent B | projection/sync tests and culprit code | Done |
| LLM planner + Chinese report | Agent | Subagent C | `crates/wiki-cli/src/orphan_governance.rs`, CLI/tests | Done |
| Apply whitelist executor | Agent | Main | governance apply path/tests | Done |
| Focused review | Agent | Reviewer | diff/spec | Done |
| Integration review + CI | Skill/Script | Main | workspace | Done |
| PR + merge follow-up | Skill | Main | roadmap, LESSONS, handoff | Pending |

## Checklist

- [x] Follow-up spec records that prior chat is the plain architecture decision.
- [x] `vault-audit` ignores non-content directories for governance stats.
- [x] `vault-audit` stops writing undated latest files.
- [x] Path-level evidence is present in audit JSON.
- [x] `orphan-governance plan` calls LLM and validates JSON.
- [x] Chinese Markdown is LLM-generated from validated plan JSON.
- [x] `orphan-governance apply` defaults to dry-run.
- [x] `--apply` only executes whitelisted actions.
- [x] Cleanup whitelist is enforced exactly.
- [x] Root `concepts/` write-back regression is covered.
- [x] `batch-ingest` is not run by B5 follow-up.
- [x] Focused review and integration review are complete.
- [x] CI passes locally.
