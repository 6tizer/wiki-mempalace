# Tasks: Audit Disposition PR3 MCP API Reference

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
| Spec trio | Script | Main | `docs/specs/audit-disposition-03-mcp-api-reference/` | PRD | Complete |
| API reference rewrite | Script | Main | `docs/mcp-api-reference.md` | spec | Complete |
| Docs sync tests | Agent | Main | `crates/wiki-cli/src/mcp.rs` tests | docs | Complete |
| Handoff and docs index | Script | Main | `docs/handovers/`, `docs/roadmap.md`, `docs/specs/README.md`, `docs/prd/audit-disposition-2026-04-30.md`, `docs/LESSONS.md` | implementation | Complete |
| Review and gates | Skill | Main | workspace | all | Complete |

## Review Notes

- Check all `tools_list()` names appear in API reference.
- Check mempalace `bank_id` is rejection/capability wording only, not client parameter docs.
- Check typed error kinds include `scope_denied`.

## Stop Conditions

- Stop if doc accuracy requires runtime schema changes.
- Stop if standalone `rust-mempalace mcp` docs need a separate reference.

## Verification

- `cargo test -p wiki-cli mcp_api_reference -- --nocapture` — passed
- `rg -n 'mempalace_.*bank_id|bank_id.*client' docs/mcp-api-reference.md` — no matches
- `git diff --check` — passed
- `cargo fmt --all -- --check` — passed
- `cargo deny --all-features check advisories bans licenses sources` — passed
- `cargo test --workspace` — passed
- `cargo clippy --workspace --all-targets -- -D warnings` — passed

## Focused Review

- Scope: on target for I-8.
- Review depth: standard.
- Hard stops: 0 found.
- Residual risk: result shape docs are high-level by design, so additive fields remain undocumented.
