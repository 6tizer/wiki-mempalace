# Requirements: Compiler Deferred Resolution Agent

## Goal

Add a machine-only resolver pass for production-wiki-compiler
`deferred_resolutions`.

## Functional Requirements

- Accept compiler JSON report via `compiler-resolve-deferred --report <json>`.
- Prefer deferred candidate `page_id`; old reports without `page_id` may resolve
  only when scope + title + entry_type uniquely identify one page.
- Support dry-run by default and apply only with `--apply`.
- Use global `--db`, `--wiki-dir`, `--sync-wiki`, `--viewer-scope`, and
  `--palace`.
- Classify each deferred item as:
  - `AliasExisting`,
  - `CreateCanonical`,
  - `IgnoreNoise`,
  - `KeepDeferred`.
- Create new canonical pages only when safe and `--allow-create` is set.
- Persist safe alias/canonical decisions to `wiki.db`.
- Project Vault only after DB update.
- Consume/replay Mempalace only after Vault projection.
- Always write lint/audit report after apply.
- Report low-confidence cases as machine-deferred, not human/manual work.

## Non-Goals

- No human/manual lane.
- No production vault regression without explicit approval.
- No direct edit to generated Markdown as source of truth.
- No direct `palace.db` mutation.
- No broad compiler run.

## Acceptance Criteria

- Dry-run changes nothing.
- Apply order is `wiki.db -> Vault projection -> Mempalace consume/replay ->
  lint/audit report`.
- Temp X + WeChat regressions pass without touching real vault.
- `--allow-create` gates all new canonical pages.
- Report records every decision and skipped reason.

## Checklist

- [x] Behavior matches PRD scope
- [x] Inputs and outputs are explicit
- [x] Out-of-scope behavior is rejected or ignored safely
- [x] Error cases are covered
- [x] Implementation complete
