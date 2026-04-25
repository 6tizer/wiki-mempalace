# PRD: Storage + Embeddings Follow-up (C16)

## Goal

- Close the two **deferred** items from the C15 code review: (1) crash-safety
  for snapshot + outbox as a **single** persistence unit, and (2) scalable
  vector similarity for `wiki_embedding` without **O(n) full table scans** at
  large corpus sizes.

## User Value

- **Atomic persist**: power loss or process crash after `save_snapshot` but
  before outbox append (or the reverse) cannot leave `wiki_state` and
  `wiki_outbox` describing different logical engine states.
- **ANN / indexed vectors**: `query` with vectors stays responsive as embedding
  rows grow; operators get predictable upper bounds for latency and memory per
  query.

## Scope

- `wiki-storage` + `wiki-kernel` + `wiki-cli` call sites that currently pair
  `save_to_repo` with `flush_outbox_to_repo*`.
- `SqliteRepository` embedding read path (`search_embeddings_cosine` and
  any future callers), optional SQLite extension (e.g. `sqlite-vec` / VSS) behind
  a feature flag.
- **Out of scope for this PRD**: `rust-mempalace` FTS/sparse pipelines unless
  the chosen ANN design naturally shares a pattern (track as a separate
  sub-task in `embedding-ann-index` spec if needed).

## Non-Goals

- Changing outbox event schema or `WikiEvent` JSON shape.
- Rewriting the entire `WikiRepository` trait without a migration story for
  in-memory or test doubles.
- Guaranteeing sub-millisecond ANN on huge corpora without hardware limits —
  the target is “bounded work per query” with documented limits.

## Success Criteria (Product)

- A single documented **unit of durability** for “commit engine work to
  SQLite” that either fully applies snapshot + outbox batch or fully rolls
  back, under process crash (best-effort validation via tests simulating
  failure points).
- Embedding search has a **documented** complexity/limit story (e.g. index +
  top-k) and does not require a full in-Rust pass over all rows for the
  default production path at scale.

## Spec Mapping

- [persist-snapshot-outbox](../specs/persist-snapshot-outbox/requirements.md)
- [embedding-ann-index](../specs/embedding-ann-index/requirements.md)

## Status

- **C16A complete** — PR #25 merged. `persist-snapshot-outbox` uses a
  `WikiRepository` method plus SQLite `BEGIN IMMEDIATE` transaction for
  snapshot + current outbox durability.
- **C16B not started** — `embedding-ann-index` remains a separate follow-up.
