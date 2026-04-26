# Requirements: Notion Sync Trusted Tag Policy

## Functional Requirements

- Add `notion-sync --tag-policy <trusted-source|strict|bootstrap>`.
- `trusted-source` is the default.
- `trusted-source` must allow any number of new tags for the current ingest.
- `trusted-source` must not reject retired/deprecated tags at ingest time.
- `strict` must leave the loaded schema unchanged.
- `bootstrap` remains supported as an alias of `trusted-source`.
- The policy change must be in-memory only and must not edit `DomainSchema.json`.
- Automation jobs that call `notion-sync` should inherit the Notion default.

## Non-Goals

- No permanent schema change.
- No Notion API behavior change.
- No tag merge/rename implementation in this PR.
