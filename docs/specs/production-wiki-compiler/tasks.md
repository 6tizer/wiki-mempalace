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

## Follow-Up: Compiler Canonicalization v2

This is the next compiler work before broad production scale-up. The tiny sample
showed that prompt-only naming is not enough; the same page can come back as
`MCP connectors`, `MCP连接器`, or `MCP 协议`. The fix should be a resolver layer,
not a growing alias list in code or prompt.

| Task | Grade | Owner | Files | Depends on | Status |
| --- | --- | --- | --- | --- | --- |
| T8 Resolver PRD/spec refresh | Script | main agent | `docs/prd/production-wiki-compiler.md`, `docs/specs/production-wiki-compiler/*`, `docs/references/notion-wiki-agent-contract.md` | T7 | Done |
| T9 Candidate retrieval design | Agent | worker | `crates/wiki-cli/src/wiki_compiler.rs`, storage/search helpers, tests | T8 | Planned |
| T10 Alias/canonical mapping storage | Agent | worker | `crates/wiki-storage/src/lib.rs`, migration/tests, compiler integration | T8/T9 | Planned |
| T11 Small LLM fallback | Agent | worker | `crates/wiki-cli/src/wiki_compiler.rs`, LLM prompt/tests | T9/T10 | Planned |
| T12 Lint/Fixer propagation contract | Agent | worker | `crates/wiki-cli/src/main.rs`, fixer/consistency docs/tests | T8 | Planned |
| T13 Production regression sample set | Skill | main agent | X/WeChat sample commands, run report, handoff | T9-T12 | Planned |

## Implementation Notes

- Prefer extracting `WikiCompilerRunner` into a new `wiki_compiler.rs` module
  instead of growing `main.rs`.
- Keep `batch-ingest` as the public job/automation entrypoint.
- Do not run production apply until implementation tests pass and the user
  approves the tiny sample.
- Do not mark Notion `已编译到Wiki` during the first sample.
- Do not scale beyond tiny samples until canonicalization v2 exists.
- Do not solve dedup by adding every observed alias to the compiler prompt.

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
- PR #46 production safety follow-up found and fixed immediate sample issues,
  but it also confirmed the need for canonicalization v2 before broad scale-up.

## Stop Conditions

- Stop and ask user if PRD scope changes.
- Stop and update spec first if implementation needs a different interface.
- Stop after 3 failed attempts at the same compile/test error and summarize
  evidence.
- Stop before production apply if backup path is not verified.
- Stop before scale-up if Obsidian human check fails.
- Stop if the resolver needs whole-wiki prompt context to decide one page.
- Stop if the proposed fix grows a manual alias list instead of adding
  candidate retrieval or persisted canonical mapping.

## Verification

- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `git diff --check`
- Production dry-run on `/Users/mac-mini/Documents/wiki`
- Production tiny sample apply only after user approval
- Mempalace consume smoke
- `query` and `query/explain --palace-db` smoke
