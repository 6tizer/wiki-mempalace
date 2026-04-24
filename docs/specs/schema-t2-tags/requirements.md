# Requirements: Schema T2 Tag Governance

## Goal

- Make `TagConfig` operational by adding tags to core models and enforcing basic ingest-time policies.

## Functional Requirements

- Add `tags: Vec<String>` to LlmClaimDraft, Claim, and Source.
- Backward-compatible serde defaults.
- Normalize tags.
- Enforce `deprecated_tags`.
- Enforce `max_new_tags_per_ingest`.

## Non-Goals

- No tag auto-extension.
- No orphan/dormant tag analytics.
- No Claim.status migration.

## Inputs / Outputs

- Input: LLM ingest plans, raw source tags, schema tag config.
- Output: normalized tags on persisted objects and vault frontmatter where applicable.

## Acceptance Criteria

- Old snapshots load.
- New ingest paths preserve normalized tags.
- Deprecated tags handled clearly.
- New tag limit enforced.

## User / Agent Gates

- User approval needed: deprecated tag behavior (error vs drop/warn).
- Agent can automate: model changes, tests, docs.
