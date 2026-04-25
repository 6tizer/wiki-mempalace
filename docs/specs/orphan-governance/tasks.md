# Tasks: B5 Orphan Governance

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
| Read production audit and define lanes | Skill / Agent | Main | `/Users/mac-mini/Documents/wiki/reports/vault-audit.json`, spec docs | B1 complete | Done |
| CLI/report implementation | Agent | Implementation subagent | `crates/wiki-cli/src/orphan_governance.rs`, `crates/wiki-cli/src/main.rs` | Spec update | Done |
| Tests | Agent | Implementation subagent | `crates/wiki-cli/tests/orphan_governance.rs`, `crates/wiki-cli/tests/vault_cli_commands.rs` | Implementation | Done |
| Handoff | Script | Main / implementation subagent | `docs/handovers/orphan-governance/summary.md` | Tests | Done |
| Focused review | Agent | Review subagent | spec + touched code | Implementation | Done |
| Status backfill | Script | Main | `docs/specs/orphan-governance/tasks.md`, `docs/specs/README.md`, `docs/roadmap.md`, `docs/LESSONS.md` | Review | Done |
| Draft PR | Skill | Main | GitHub PR #28 | Gate pass | Done |
| GitHub CI quick | Skill | Main | PR #28 `quick` check | Draft PR | Done |

## Review Notes

- B5 must not mutate vault content outside report files.
- Treat 2026-04-25 audit as source of truth.
- Do not create apply mode in this PR.
- Review must check JSON/Markdown source-of-truth relationship.
- Review must check path rules with `--wiki-dir`.

## Stop Conditions

- Stop before any vault mutation.
- Stop if the audit report is missing or malformed.
- Stop if implementation needs DB/palace access.

## Verification

- `cargo fmt --all -- --check`
- `cargo test -p wiki-cli --test orphan_governance`
- `cargo test -p wiki-cli --test vault_cli_commands`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
