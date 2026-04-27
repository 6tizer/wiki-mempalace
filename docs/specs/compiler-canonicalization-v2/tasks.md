# Tasks: Compiler Canonicalization v2

## Checklist

- [x] Requirements drafted
- [x] Design drafted
- [x] Plain architecture captured from user request
- [x] Branch created
- [x] Tasks graded as Script / Skill / Agent
- [x] Storage alias mapping implemented
- [x] Candidate resolver implemented
- [x] Small LLM fallback implemented
- [x] Regression fixtures/smoke implemented
- [x] Module handoff written
- [x] Focused review complete
- [x] Integration review complete
- [x] Tests added/updated
- [x] Docs updated
- [x] PR opened
- [x] CI green
- [x] PR merged
- [x] Post-merge docs backfilled

## Plan Mode Grading

| Task | Grade | Owner | Files | Depends on | Status |
| --- | --- | --- | --- | --- | --- |
| T0 PRD/spec split | Script | main agent | `docs/prd/compiler-canonicalization-v2.md`, `docs/specs/compiler-canonicalization-v2/*`, indexes | none | Done |
| T1 Storage alias mapping | Agent | worker-storage | `crates/wiki-storage/src/lib.rs`, storage tests | T0 | Done |
| T2 Resolver/candidate retrieval | Agent | worker-resolver/main agent | `crates/wiki-cli/src/wiki_compiler.rs`, compiler tests | T0/T1 | Done |
| T3 Small LLM fallback | Agent | worker-resolver/main agent | `crates/wiki-cli/src/wiki_compiler.rs`, `crates/wiki-cli/src/llm.rs` if needed | T2 | Done |
| T4 Regression temp-vault smoke | Agent | main agent | `crates/wiki-cli/src/wiki_compiler.rs`, CLI/integration tests, handoff evidence | T2/T3 | Done |
| T5 Focused review | Skill | review subagent/main agent | changed files | T1-T4 | Done |
| T6 Integration gate | Skill | main agent | workspace | T5 | Done |
| T7 Post-merge status backfill | Script | main agent | `docs/roadmap.md`, `docs/LESSONS.md`, PRD/tasks/handoff status | PR merge | Done |

## Owner Boundaries

- `worker-storage` owns alias mapping structs/table/repository methods.
- `worker-resolver` owns compiler resolver, candidate ranking, fallback prompt,
  and page materialization integration.
- `worker-regression` owns temporary fixture tests and handoff evidence only.
- No worker may mutate real production vault.
- No worker may edit another worker's files unless main agent explicitly
  integrates after return.

## Stop Conditions

- Stop if resolver needs all pages in prompt.
- Stop if implementation adds many source-specific aliases to compiler code.
- Stop if low-confidence resolution still creates a page.
- Stop before production vault apply.
- Stop if storage schema needs a PRD scope change.

## Verification

- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `git diff --check`
- Temporary X/WeChat-shaped regression, no real vault mutation.
