# Tasks: Production Wiki Compiler

## Checklist

- [x] Requirements drafted
- [x] Design drafted
- [x] Plain architecture approved
- [x] Spec approved
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
- [x] Codex/GitHub review addressed
- [x] CI green
- [x] Merged
- [x] Roadmap/PRD updated

## Subtasks

| Task | Grade | Owner | Files | Depends on | Status |
| --- | --- | --- | --- | --- | --- |
| T0 Spec finalization | Script | main agent | `docs/prd/production-wiki-compiler.md`, `docs/specs/production-wiki-compiler/*` | none | Done |
| T1 Compiler plan contract | Agent | worker | `crates/wiki-core/src/llm_ingest_plan.rs`, tests/fixtures | T0 | Done |
| T2 Compiler runner | Agent | worker | `crates/wiki-cli/src/main.rs`, `crates/wiki-cli/src/wiki_compiler.rs` | T1 | Done |
| T3 Page materialization | Agent | worker/main agent | `crates/wiki-kernel/src/*`, `crates/wiki-cli/src/*`, `docs/vault-standards.md` | T1 | Done |
| T4 Dedup and references | Agent | worker/main agent | `crates/wiki-cli/src/wiki_compiler.rs`, related tests | T2/T3 | Done |
| T5 CLI and automation compatibility | Agent | worker | `crates/wiki-cli/src/main.rs`, automation tests | T2/T4 | Done |
| T6 Production tiny sample runbook | Skill | main agent | `docs/handovers/production-wiki-compiler/summary.md`, run report | T1-T5 | Done |
| T7 Review and verification | Skill | review subagent/main agent | tests, clippy, production dry-run evidence | T1-T6 | Done |

## Implementation Notes

- Prefer extracting `WikiCompilerRunner` into a new `wiki_compiler.rs` module
  instead of growing `main.rs`.
- Keep `batch-ingest` as the public job/automation entrypoint.
- Do not run production apply until implementation tests pass and the user
  approves the tiny sample.
- Do not mark Notion `已编译到Wiki` during the first sample.

## Review Notes

- Review must compare output against the Notion Wiki Compiler Instructions, not
  against the old summary-only implementation.
- Worker A implemented the richer `LlmIngestPlanV1` contract with backwards
  compatibility.
- Worker B extracted `wiki_compiler.rs` and kept `batch-ingest` as the public
  entrypoint.
- Main integration fixed `PageWritten` emission and source id reuse for
  projected Notion source files.
- Main integration fixed focused review findings:
  - same-title summaries no longer overwrite each other in Vault projection,
  - source frontmatter `tags:` supports inline and YAML block lists,
  - relationship entity lookup is scoped,
  - rich `summary.confidence` is preserved in page metadata.
- PR #44 merged on 2026-04-27. Production apply was intentionally not run in
  the implementation PR.
- Remaining gate is operational, not implementation: run a backed-up tiny
  production sample before any scale-up.

## Stop Conditions

- Stop and ask user if PRD scope changes.
- Stop and update spec first if implementation needs a different interface.
- Stop after 3 failed attempts at the same compile/test error and summarize
  evidence.
- Stop before production apply if backup path is not verified.
- Stop before scale-up if Obsidian human check fails.

## Verification

- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `git diff --check`
- Production dry-run on `/Users/mac-mini/Documents/wiki`
- Production tiny sample apply only after user approval
- Mempalace consume smoke
- `query` and `query/explain --palace-db` smoke
