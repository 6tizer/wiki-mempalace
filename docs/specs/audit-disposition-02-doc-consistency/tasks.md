# Tasks: Audit Disposition PR2 Doc Consistency

## Checklist

- [x] Requirements approved
- [x] Design approved
- [x] Plain architecture approved
- [x] Plan approved
- [x] Branch created
- [x] Tasks graded as Script / Skill / Agent
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
- [x] Roadmap/PRD updated

## Subtasks

| Task | Grade | Owner | Files | Depends on | Status |
| --- | --- | --- | --- | --- | --- |
| Spec trio | Script | Main | `docs/specs/audit-disposition-02-doc-consistency/` | PRD | Complete |
| Edition docs cleanup | Script | Main | `docs/architecture.md`, `crates/rust-mempalace/README.md` | spec | Complete |
| Notion status wording check | Script | Main | active docs | spec | Complete |
| Handoff and docs index | Script | Main | `docs/handovers/`, `docs/roadmap.md`, `docs/specs/README.md`, `docs/prd/audit-disposition-2026-04-30.md`, `docs/LESSONS.md` | implementation | Complete |
| Review and gates | Skill | Main | workspace | all | Complete |

## Review Notes

- Check that archive-only stale text is not treated as current truth.
- Check that docs do not imply a Cargo behavior change.

## Stop Conditions

- Stop if code manifests contradict the docs.
- Stop if fixing active docs would require PRD scope expansion.

## Verification

- active-doc grep for old edition wording — passed
- active-doc grep for stale Notion sync not-implemented wording — passed
- `git diff --check` — passed
- `cargo fmt --all -- --check` — passed
- `cargo deny --all-features check advisories bans licenses sources` — passed
- `cargo test --workspace` — passed
- `cargo clippy --workspace --all-targets -- -D warnings` — passed

## Focused Review

- Scope: on target for L-4.
- Review depth: standard.
- Hard stops: 0 found.
- Residual risk: archive docs still contain historical wording by design.
