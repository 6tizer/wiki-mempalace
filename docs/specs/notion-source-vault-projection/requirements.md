# Requirements: Notion Source Vault Projection

## Functional Requirements

- Project only `RawArtifact` records whose URI starts with `notion://`.
- Map `notion://x_bookmark/<id>` to `sources/x/`.
- Map `notion://wechat/<id>` to `sources/wechat/`.
- Source Markdown must include:
  - `title`
  - `kind: source`
  - `origin`
  - `url`
  - `origin_label`
  - `compiled_to_wiki: false`
  - `created_at`
  - `source_id`
  - `notion_uuid`
  - `tags`
- Existing files are detected by `source_id` or `notion_uuid`.
- The operation must be idempotent.
- Projected `tags` must be Obsidian-safe tag names, because the `tags`
  frontmatter key is rendered by Obsidian as a strict tag property.
- `notion-source-vault-sync` defaults to dry-run and requires `--apply` to write.
- `notion-source-vault-sync --repair-tags` must rewrite existing source
  frontmatter tags into Obsidian-safe names without mutating `wiki.db`.
- `notion-sync --sync-wiki` must run the same source projection after successful
  DB writes.

## Non-Goals

- No mempalace source drawers.
- No summary page generation.
- No Notion archived retirement.
- No DB mutation beyond existing `notion-sync` behavior.
