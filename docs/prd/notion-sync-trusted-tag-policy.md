# PRD: Notion Sync Trusted Tag Policy

**Status**: active

## Goal

Make wiki ingest inherit Notion AI auto-fill tags instead of treating them as
untrusted tag creation.

## Context

Notion AI auto-fill already acts as an upstream tag classifier. Its prompt asks
for 2-3 labels across scenario, technical method, and product form, and tells
the classifier to avoid retired broad tags.

That means Notion tags are not arbitrary user noise. For this pipeline, they are
source metadata produced by the upstream knowledge system.

## Problem

The wiki schema currently uses `max_new_tags_per_ingest = 1` as a strict daily
guardrail. That is useful for local LLM-generated tags, but too restrictive for
Notion sync. A single Notion page can legitimately arrive with several tags.

In production, historical `x_bookmark` sync failed until we used a temporary
schema copy with a higher tag limit. The better system behavior is to make
Notion sync lenient by default.

## Scope

- Add `notion-sync --tag-policy trusted-source`.
- Make `trusted-source` the default for `notion-sync`.
- Keep `strict` available for debugging and conservative runs.
- Keep `bootstrap` as an alias for historical catch-up wording.
- In trusted-source mode, do not block ingest on the number of new tags.
- In trusted-source mode, do not block ingest on retired/deprecated tags; keep
  those as later governance/reporting input instead.
- Do not edit `DomainSchema.json`.

## Success Criteria

- Daily Notion sync accepts Notion AI auto-fill tags by default.
- Strict mode remains available and keeps the existing schema behavior.
- Tests prove trusted-source accepts source tags without mutating the schema file.
