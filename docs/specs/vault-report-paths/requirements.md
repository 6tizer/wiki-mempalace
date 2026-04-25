# Requirements: Vault Report Paths

## Problem

Report-like CLI outputs can still land under the process working directory when
an Agent passes defaults or relative paths. This can create repo-local
`wiki/reports/` artifacts instead of writing to the active Obsidian vault.

## Scope

- `wiki-cli dashboard`
- `wiki-cli suggest --report-dir`
- `wiki-cli metrics --report`
- `wiki-cli automation health --summary-file`

## Requirements

- Absolute output paths remain unchanged.
- When `--wiki-dir <PATH>` is present, relative report output paths resolve
  under that wiki root.
- When `--wiki-dir` is absent, explicit relative output paths keep existing
  current-working-directory behavior.
- `dashboard` with no `--output` writes to:
  - `<wiki-dir>/reports/dashboard.html` when `--wiki-dir` is present.
  - `wiki/reports/dashboard.html` when `--wiki-dir` is absent.
- `suggest --report-dir` with no value writes to:
  - `<wiki-dir>/reports/suggestions` when `--wiki-dir` is present.
  - `wiki/reports/suggestions` when `--wiki-dir` is absent.
- Parent directories are created before writing report files.

## Non-goals

- Do not change `--db`, `--llm-config`, `schema-validate`, or mempalace bank
  defaults.
- Do not change projection ownership rules.
- Do not write dashboard or suggestion reports unless the command already
  requests them.

## Acceptance

- Tests cover vault-relative defaults for dashboard and suggest.
- Tests cover vault-relative explicit relative paths for metrics and automation
  health summary.
- Existing absolute-path report tests still pass.
