# wiki-agent 06 Memory + Skills Requirements

## Scope

PR6 adds the first durable memory loop for `wiki-agent`.

## Functional Requirements

- Keep raw chat transcript in `.wiki/wiki-agent.db`.
- Add `EntryType::Skill`.
- Project skill pages to `pages/skill/`.
- Include `Skill` pages in Mempalace page ingestion.
- Extract long-term candidates only from explicit user intent:
  - `记住：...`
  - `remember: ...`
  - `技能：...`
  - `skill: ...`
- Reject memory candidates containing credentials, prompt-injection language, or
  invisible control characters.
- Prevent exact duplicate durable memories.
- Write verified fact/preference memories as concept pages.
- Write verified skills as `entry_type=skill` pages with fixed sections:
  - `触发条件`
  - `操作步骤`
  - `输入输出`
  - `验证方式`
  - `失败处理`
- `wiki-agent chat` should run the curator at session close.
- `wiki-agent memory curate --session <id> --apply` should run the same curator manually.
- PR5 `memory_curator` worker should call the curator instead of returning blocked.

## Non-Goals

- No LLM-based implicit memory mining yet.
- No automatic extraction from ordinary conversation without explicit marker.
- No credential storage.

## Acceptance

- Exact duplicate memory does not create another page.
- Injection / credential / invisible Unicode memory is rejected.
- Skill candidate writes a `Skill` page under `pages/skill/`.
- Manual memory curate returns stable JSON.
- `consume-to-mempalace` can ingest Skill pages.
