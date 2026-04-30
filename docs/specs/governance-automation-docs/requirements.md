# Requirements: Governance Automation Docs

## Scope

PR7 closes Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3 by
wiring the new governance/Fixer/Synthesis commands into automation and syncing
operator-facing docs.

## Functional Requirements

- `wiki-cli automation list-jobs` includes:
  - `governance-scan`
  - `fixer-plan`
  - `fixer-apply`
  - `synthesis-discover`
  - `synthesis-run`
- Daily maintenance order is:
  `notion-sync -> batch-ingest -> governance-scan -> fixer-plan -> fixer-apply -> maintenance -> consume-to-mempalace -> vault-reports`.
- `lint` remains a manual automation job; daily uses `governance-scan` and
  `maintenance` instead of a standalone lint step.
- `fixer-apply` uses the latest fixer plan when present; if no plan exists, it
  creates one before applying ready `evidence-auto` actions.
- Synthesis automation is manual:
  - `synthesis-discover` is read-only.
  - `synthesis-run` discovers current candidates and applies verified
    `in_review/high` synthesis pages.
- Reports are written under vault-relative directories when `--wiki-dir` is set:
  - `reports/governance/`
  - `reports/fixer/`
  - `reports/synthesis/`

## Docs Requirements

- Update roadmap, PRD, spec index, architecture, MCP API reference,
  vault standards, dev workflow, `llm-config.example.toml`, handoff, and
  `LESSONS.md`.
- Add a Notion workflow capability matrix that explains what is covered locally,
  what is intentionally CLI-only, and what remains outside this batch.

## Non-Goals

- Do not add governance/Fixer/Synthesis MCP write tools in this PR.
- Do not run production `/Users/mac-mini/Documents/wiki` apply commands.
- Do not change QA behavior.
