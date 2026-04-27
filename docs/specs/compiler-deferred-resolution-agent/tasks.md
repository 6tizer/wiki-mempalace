# Tasks: Compiler Deferred Resolution Agent

## Checklist

- [x] PRD drafted
- [x] Spec requirements drafted
- [x] Spec design drafted
- [x] Spec tasks drafted
- [x] Branch created
- [x] Plan mode completed
- [x] Subagent assigned
- [x] Handoff seeded
- [x] CLI command implemented
- [x] Decision classifier implemented
- [x] Apply path implemented
- [x] Temp X + WeChat regression added
- [x] Focused review complete
- [x] Integration review complete
- [x] Tests added/updated
- [x] Handoff updated with verification
- [ ] PR opened
- [ ] CI green
- [ ] PR merged

## Plan Mode Grading

| Task | Grade | Owner | Files | Depends on | Status |
| --- | --- | --- | --- | --- | --- |
| T0 PRD/spec/handoff seed | Agent | current agent | `docs/prd/compiler-deferred-resolution-agent.md`, `docs/specs/compiler-deferred-resolution-agent/*`, `docs/handovers/compiler-deferred-resolution-agent/summary.md` | none | Done |
| T1 CLI parse/report IO | Agent | main agent | `crates/wiki-cli/*`, tests | T0 | Done |
| T2 Machine decision classifier | Agent | main agent | compiler resolver/CLI modules, tests | T1 | Done |
| T3 Apply order integration | Agent | main agent | storage/projection/mempalace/lint call sites | T2 | Done |
| T4 Temp regression | Agent | main agent | temp fixtures/tests/handoff evidence | T1-T3 | Done |
| T5 Focused review | Skill | main agent/check skill | changed implementation files | T1-T4 | Done |
| T6 Integration gate | Skill | main agent | workspace | T5 | Done |

## Owner Boundaries

- Current doc seed touches only listed owner docs.
- Implementation subagent must claim code files before editing.
- No agent may mutate real production vault during regression.

## Verification Planned

- `cargo fmt --all -- --check`
- `cargo test -p wiki-cli compiler_resolve_deferred` - passed
- temp X + WeChat apply smoke - passed, no `page.broken_wikilink`
- `cargo test --workspace` - passed
- `cargo clippy --workspace --all-targets -- -D warnings` - passed
- `git diff --check` - passed
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `git diff --check`
- Temp X + WeChat regression only.
