# Requirements: B5 Orphan Governance

## Goal

- Produce a read-only governance report from the fresh production
  `vault-audit.json`.
- Classify audit findings into action lanes without mutating the vault.

## Plain-Language Summary

- What this module does: reads the audit report and says what kind of cleanup is
  safe to discuss next.
- Who it talks to: `vault-audit.json`; it does not edit Markdown files.
- What user decision it implements: B5 reports and classifies only. It does not
  clean, rewrite, move, or flip source/page metadata.

## Current Production Evidence

Input report:

- `/Users/mac-mini/Documents/wiki/reports/vault-audit.json`
- generated after the 2026-04-25 production backfill and palace init.

Observed counts:

- `orphan_candidates.total_files = 4`
- `readiness.unsupported_frontmatter = 12`
- `pages.missing_status = 5`
- `sources.compiled_to_wiki.missing = 16`

Current orphan samples:

- `.wiki/orphan-audit-report.md`
- `_archive/legacy-root/AGENTS md 5da673ca2377484498ec12f5679bfbf3.md`
- `_archive/legacy-root/README.md`
- `_archive/legacy-root/concepts/04ff4434.md`

## Functional Requirements

- Add a read-only `wiki-cli orphan-governance` command.
- Accept an audit JSON path with `--audit-report <PATH>`.
- Default report directory:
  - with `--wiki-dir`: `<wiki-dir>/reports/`
  - without `--wiki-dir`: audit report parent directory.
- Emit JSON as source of truth plus Markdown sibling report.
- Classify findings into these lanes:
  - `report_only`
  - `future_auto_fix`
  - `agent_review`
  - `human_required`
- For the 2026-04-25 report, classify:
  - 4 orphan candidates: `human_required`, because all samples are old
    `.wiki`/`_archive` or unclassified Markdown and deletion/move would be
    irreversible.
  - 12 unsupported frontmatter files: `agent_review`, because the audit only
    exposes the count as readiness data in this report; path-level evidence is
    required before any fix.
  - 5 pages missing `status`: `future_auto_fix`, but report-only in v1. Future
    dry-run may propose `status: draft` only after exact page paths are listed.
  - 16 sources missing `compiled_to_wiki`: `agent_review`, because setting
    `true` can hide uncompiled work and setting `false` can trigger unwanted
    recompile.
- Include explicit mutation policy:
  - v1 writes reports only.
  - no vault Markdown mutation.
  - no DB mutation.
  - no outbox emission.
  - no palace write.
- Include source audit report metadata and counts in output.

## Non-Goals

- Do not clean vault files.
- Do not move files out of `_archive`.
- Do not delete old reports.
- Do not add `status`.
- Do not add or flip `compiled_to_wiki`.
- Do not rerun LLM.
- Do not require `wiki.db` or `palace.db`.

## Inputs / Outputs

- Input: `vault-audit.json`.
- Output:
  - `orphan-governance-report.json`
  - `orphan-governance-report.md`

## Acceptance Criteria

- Command succeeds against `/Users/mac-mini/Documents/wiki/reports/vault-audit.json`.
- Output JSON includes the 4/12/5/16 counts from the audit report.
- Output Markdown states which findings are report-only, future auto-fix,
  agent-review, and human-required.
- Running the command does not change any vault Markdown outside `reports/`.
- Tests cover classification and read-only report writing.

## Checklist

- [x] Behavior matches PRD scope
- [x] Inputs and outputs are explicit
- [x] Out-of-scope behavior is rejected or ignored safely
- [x] Error cases are covered

## User / Agent Gates

- User approval needed: any future apply mode, archive move, deletion,
  frontmatter mutation, or LLM recompile.
- Agent can automate: report reading, classification, report writing, tests,
  and code review.
