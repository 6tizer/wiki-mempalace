# PRD: Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3

## Goal

Build a native maintenance and research loop for the local wiki system instead
of cloning Notion agents one-by-one.

The system model remains:

```text
wiki.db -> Vault projection -> Mempalace projection
```

## Scope

- Add task-specific LLM profiles so Fixer, Synthesis, QA, and verifier paths can
  use different providers, models, limits, and reasoning settings.
- Add multi-provider web search infrastructure for cross-verification.
- Add governance scan, evidence fixer plan/apply/restore, synthesis discovery,
  web-backed synthesis composition, and automation/docs follow-up across the
  seven planned PRs.
- Preserve existing CLI/MCP behavior unless a new command is explicitly used.

## Non-Goals

- Do not replace `wiki.db` as the source of truth.
- Do not directly mutate generated Vault Markdown or `palace.db`.
- Do not include QA rewrite in this batch beyond boundary documentation.
- Do not run production `/Users/mac-mini/Documents/wiki` apply operations as
  part of implementation verification.

## PR Sequence

| PR | Branch | Scope | Status |
| --- | --- | --- | --- |
| 1 | `codex/ai-provider-profiles` | LLM profiles + dual web search provider runtime | Merged PR #101 |
| 2 | `codex/wiki-governance-scan` | Unified governance scan | Merged PR #102 |
| 3 | `codex/evidence-fixer-plan` | Evidence Fixer typed dry-run plan | Local complete, pending PR |
| 4 | `codex/evidence-fixer-apply-restore` | Evidence Fixer apply + restore | Planned |
| 5 | `codex/synthesis-discovery` | Synthesis point/line/plane/volume discovery | Planned |
| 6 | `codex/web-backed-synthesis-composer` | Web-backed synthesis writing + verification | Planned |
| 7 | `codex/governance-automation-docs` | Automation, MCP, and docs closeout | Planned |

## Success Criteria

- Existing `[llm]` config remains compatible.
- New LLM profiles can be selected by task.
- Exa and Tavily web search providers can run the same query and produce a
  shared evidence artifact.
- Future Fixer/Synthesis PRs can reuse the same LLM/search infrastructure.
- Final closeout includes Notion workflow parity mapping, roadmap, specs,
  handovers, MCP docs, and `LESSONS`.
