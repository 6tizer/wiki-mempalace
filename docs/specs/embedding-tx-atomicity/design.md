# Design: Embedding Tx Atomicity

## Storage API

Add an owned embedding write model:

```rust
pub struct EmbeddingWrite {
    pub doc_id: String,
    pub vector: Vec<f32>,
}
```

Add a SQLite-specific commit API:

```rust
SqliteRepository::save_snapshot_and_append_outbox_with_embeddings(
    snapshot,
    events,
    embeddings,
)
```

The method runs:

1. `BEGIN IMMEDIATE`
2. snapshot upsert into `wiki_state`
3. ordered inserts into `wiki_outbox`
4. upserts into `wiki_embedding`
5. `COMMIT`

Any error rolls back the full unit.

## Callers

Vector-enabled callers build embeddings before committing:

- `wiki-cli ingest`
- `wiki-cli ingest-llm`
- `wiki-cli file-claim`
- `wiki-cli supersede-claim`
- MCP equivalents
- compiler batch ingest source and claim paths

Non-vector callers keep the existing engine save+flush method.

## Error Semantics

When `--vectors` / MCP vectors is enabled, embedding failure is no longer best-effort for writes that request embeddings. The command returns an error before committing state, or rolls back the full DB transaction if SQLite embedding insert fails.

## Boundary

Vault projection and frontmatter edits are outside SQLite transaction scope. DB remains the source of truth; projection failures keep existing behavior and surface as errors after commit.

## Tests

- Success test: snapshot, outbox, and embedding are visible together.
- Failure test: SQLite trigger rejects an embedding insert; old snapshot remains, no outbox row is written, no embedding row is visible.
