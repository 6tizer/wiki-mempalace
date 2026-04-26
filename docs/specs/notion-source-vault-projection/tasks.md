# Tasks: Notion Source Vault Projection

- [x] Confirm roadmap adjacent items and split scope from archived retirement.
- [x] Add DB-backed Notion source projection helper.
- [x] Add `notion-source-vault-sync` dry-run/apply command.
- [x] Wire `notion-sync --sync-wiki` to project source Markdown after writes.
- [x] Add idempotency tests for projection.
- [x] Run production dry-run and confirm 176 DB-backed Notion sources, 172 unique source files to write, and 4 duplicate Notion UUID records de-duplicated in-run.
- [x] Run focused tests, workspace tests, clippy, and diff checks.
- [x] Open initial PR #41 and defer production `--apply` to post-review execution.
- [x] Post-merge Obsidian check found invalid tag display for labels such as `Apache2.0`; added Obsidian-safe tag projection and `--repair-tags`, then repaired production Vault source tags.
- [x] Follow-up check found newly synced Notion sources had property-only bodies; added Notion block content fetch plus `--refresh-existing` DB/Vault refresh path.
- [x] Merge PR #42.
- [x] Run production 30-day refresh window: X fetched 782 / refreshed 108; WeChat fetched 485 / refreshed 53; errors 0.
- [x] Apply refreshed source projection for 158 source files.
- [x] Verify final dry-run: `notion-source-vault-sync --dry-run --refresh-existing --repair-tags` reports planned 0 and tags_rewritten 0.
- [x] Verify source body/tag health: DB-backed Notion sources 176, body length >=1000 for 171 sources, invalid Obsidian tags 0.
- [x] User confirmed Obsidian `sources/x` and `sources/wechat` articles now show body content.
- [x] Defer LLM compilation of Notion raw sources to the next workflow.
