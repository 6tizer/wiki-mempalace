# Notion Workflow Capability Matrix

This maps the Notion maintenance workflow to the local wiki system as of the
Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3 batch.

| Notion role / workflow | Local capability | Coverage |
| --- | --- | --- |
| Source intake | `notion-sync`, `batch-ingest`, `ingest`, `ingest-llm` | Covered. New content enters `wiki.db`; Vault is projection. |
| Compiler | `batch-ingest`, compiler deferred resolver, `--sync-wiki` | Covered. Source Markdown becomes summary/concept/entity pages through DB-first writes. |
| Fixer | `governance scan`, `governance fixer-plan`, `governance fixer-apply`, `governance restore` | Covered. High-evidence fixes can apply automatically; destructive/merge actions have tombstones. |
| Synthesis | `research-synthesis discover`, `compose`, `run` | Covered. Uses internal wiki evidence plus dual-provider web evidence unless explicitly internal-only. |
| QA | Existing `qa` page path and query/write tools | Boundary documented. This batch does not redesign QA approval behavior. |
| Chat operator | `wiki-agent chat` | Covered. Uses local wiki/palace evidence first, optional Exa/xAI web evidence, and task-specific LLM profiles. |
| TUI operator | `wiki-agent tui` / `wiki-agent chat --tui` | Covered. Ratatui/crossterm terminal UI with conversation, activity, input, status, history, and keyboard navigation. |
| Sub-agent orchestration | `wiki-agent agent run` | Covered. Manager routes to native Lint/Governance/Fixer/Synthesis/Search/Memory workers; writes stay behind writer lease. |
| Persistent memory / skills | `wiki-agent memory curate/status/search` plus chat close curator | Covered. Raw sessions stay in `.wiki/wiki-agent.db`; verified facts/skills become `wiki.db` pages and then palace drawers. |
| Projection | `write_projection`, `--sync-wiki`, `vault-reports` | Covered. Managed pages are regenerated from DB; hand-written pages are protected. |
| Mempalace memory | `consume-to-mempalace`, MCP `mempalace_*` read/extract tools | Covered. Palace is an outbox-fed projection, not a source of truth. |
| Daily maintenance | `automation run-daily` | Covered. Runs sync, compile, governance scan, fixer plan/apply, maintenance, consume, reports. |
| Research maintenance | `automation run synthesis-discover`, `automation run synthesis-run` | Covered as manual automation jobs; not part of daily lane. |
| External verification | LLM profiles plus Exa/xAI web search policies | Covered for Fixer/Synthesis paths that need outside evidence. |

## Practical Meaning

- Normal ingestion and maintenance can now run from one local command:
  `wiki-cli ... automation run-daily`.
- Synthesis is not just tag counting: discovery finds candidates; compose uses
  internal evidence, external search, draft generation, and verifier checks.
- MCP remains for live agent interaction. Batch governance and synthesis runs
  stay on CLI because they produce audit artifacts and need stronger operator
  visibility.
- `wiki-agent` is now the local interactive surface. It uses native Rust calls
  by default; MCP child-process access is only a compatibility fallback.
