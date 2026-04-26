# DB/Vault Source Link Candidate Handoff

## Scope

- Task: C / DB-backed stale link and source-summary candidate helpers.
- Owner files touched:
  - `crates/wiki-cli/src/consistency.rs`
  - `crates/wiki-cli/tests/consistency.rs`
  - `docs/handovers/db-vault-palace-consistency/source-links.md`
- No production vault mutation.
- No apply logic.
- No LLM or fuzzy auto-repair.

## Implemented Helpers

- `find_stale_notion_link_candidates`
  - Scans DB page markdown evidence.
  - Extracts wikilinks `[[...]]` and Markdown local links `[text](target)`.
  - Ignores `http://` and `https://`.
  - Emits `candidate_only` when target has URL encoding, for example `AGENTS%20md`.
  - Emits `candidate_only` when decoded filename looks like old Notion export:
    `Title <32-hex>.md`.
  - Does not rewrite markdown.

- `find_source_summary_candidates`
  - Scans DB source evidence and DB summary page evidence.
  - Exact URL candidate only when source `uri` equals summary frontmatter
    `source_url`, `source_uri`, `uri`, or `url`.
  - Exact title candidate only when source frontmatter `title` equals summary
    frontmatter `source_title` / `title`, or summary DB title after stripping
    `摘要：` / `Summary: `.
  - Similar titles without exact evidence emit `NeedsHuman` with `deferred`.

## Limits

- Helpers are pure candidate builders; A/B/main wiring still needs to feed DB
  snapshot pages/sources into these structs.
- No source-summary relation is applied automatically.
- Fuzzy or partial title overlap is never executable evidence.
- Old Notion-style links are only reported as stale candidates because target
  intent cannot be proven from filename alone.

## Verification

```bash
cargo test -p wiki-cli --test consistency
```

Result: 3 passed.
