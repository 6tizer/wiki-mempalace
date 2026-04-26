# Design: Notion Source Vault Projection

## Commands

Backfill existing DB-backed Notion sources:

```bash
wiki-cli notion-source-vault-sync --vault /Users/mac-mini/Documents/wiki --apply
```

Default dry-run:

```bash
wiki-cli notion-source-vault-sync --vault /Users/mac-mini/Documents/wiki
```

Repair existing source tags for Obsidian's stricter `tags` property:

```bash
wiki-cli notion-source-vault-sync --vault /Users/mac-mini/Documents/wiki --repair-tags --apply
```

Refresh existing DB-backed source Markdown after `wiki.db` source bodies change:

```bash
wiki-cli notion-source-vault-sync --vault /Users/mac-mini/Documents/wiki --refresh-existing --apply
```

Future incremental sync:

```bash
wiki-cli --wiki-dir /Users/mac-mini/Documents/wiki --sync-wiki notion-sync --db-id all
```

Refresh existing Notion sources from Notion API blocks and reproject them:

```bash
wiki-cli --wiki-dir /Users/mac-mini/Documents/wiki --sync-wiki \
  notion-sync --db-id all --refresh-existing
```

## Projection

Projection starts from `wiki.db` snapshot sources, not from Notion API.

```text
RawArtifact { uri: notion://wechat/<page_id>, body, tags }
    -> sources/wechat/<slug(title)>.md
RawArtifact { uri: notion://x_bookmark/<page_id>, body, tags }
    -> sources/x/<slug(title)>.md
```

Filename slug follows vault-standards: keep Chinese characters, fold whitespace
and punctuation to `-`, max 80 chars. If a file path conflicts with another
source, append the source id prefix.

Tags are written as Obsidian-safe tag names in frontmatter. The projector keeps
letters, numbers, Chinese characters, `/`, `_`, and `-`, and folds other
separators to `-`. Examples: `Apache2.0 -> Apache2-0`,
`API Key -> API-Key`, `Apple Silicon -> Apple-Silicon`.

## Idempotency

Before planning a write, scan `sources/**/*.md` and index `source_id` plus
`notion_uuid`. If either identity already exists, count the source as existing.
By default existing files are not rewritten. With `--refresh-existing`, the
projector renders the current DB source body/tags and rewrites only files whose
content differs.

## Mempalace

This PR keeps raw source text out of mempalace. Existing docs intentionally say
`SourceIngested` is a no-op for live palace consumption. If source drawers are
needed later, that needs a separate PRD because it changes search corpus size and
privacy surface.
