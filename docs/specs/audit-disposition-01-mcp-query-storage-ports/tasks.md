# Tasks: Audit Disposition PR1 MCP Query Storage Ports

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
| PRD + spec | Script | Main | `docs/prd/`, `docs/specs/audit-disposition-01-mcp-query-storage-ports/` | none | Complete |
| MCP storage-backed query helper | Agent | Main | `crates/wiki-cli/src/mcp.rs` | spec | Complete |
| MCP focused regression tests | Agent | Main | `crates/wiki-cli/src/mcp.rs` tests | implementation | Complete |
| Handoff and docs index | Script | Main | `docs/handovers/`, `docs/roadmap.md`, `docs/specs/README.md`, `docs/prd/README.md`, `docs/LESSONS.md` | implementation | Complete |
| Review and gates | Skill | Main | workspace | all | Complete |

## Review Notes

- Check that `QueryServed` persistence cannot overwrite repo with stale memory.
- Check that invalid palace fallback does not hide storage results.

## Stop Conditions

- Stop and update spec first if MCP result schema must change.
- Stop if query persistence requires new storage API.

## Verification

- `cargo test -p wiki-cli mcp_query -- --nocapture` — passed
- `git diff --check` — passed
- `cargo fmt --all -- --check` — passed
- `cargo deny --all-features check advisories bans licenses sources` — passed
- `cargo test --workspace` — passed
- `cargo clippy --workspace --all-targets -- -D warnings` — passed

## Focused Review

- Scope: on target for M-5.
- Review depth: standard.
- Hard stops: 0 found.
- Residual risk: storage reload failure still falls back to old memory path by design.
