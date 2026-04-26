# Tasks: Notion Source Vault Projection

- [x] Confirm roadmap adjacent items and split scope from archived retirement.
- [x] Add DB-backed Notion source projection helper.
- [x] Add `notion-source-vault-sync` dry-run/apply command.
- [x] Wire `notion-sync --sync-wiki` to project source Markdown after writes.
- [x] Add idempotency tests for projection.
- [x] Run production dry-run and confirm 176 DB-backed Notion sources, 172 unique source files to write, and 4 duplicate Notion UUID records de-duplicated in-run.
- [x] Run focused tests, workspace tests, clippy, and diff checks.
- [x] Open PR #41 and defer production `--apply` until merge.
