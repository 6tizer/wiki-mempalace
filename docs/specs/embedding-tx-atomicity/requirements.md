# Requirements: Embedding Tx Atomicity

## Functional Requirements

- `SqliteRepository` MUST expose an API that writes `StorageSnapshot`, ordered `WikiEvent` outbox rows, and embedding rows inside one SQLite transaction.
- CLI vector write paths MUST compute embeddings before committing the logical source/claim operation.
- MCP vector write paths MUST compute embeddings before committing the logical source/claim operation.
- Compiler vector write paths MUST commit source/claim state and embeddings in the same SQLite transaction.
- If an embedding insert fails after snapshot/outbox writes, the transaction MUST roll back snapshot and outbox writes.
- Non-vector write paths MUST retain existing behavior.

## Non-Functional Requirements

- The implementation MUST NOT add a new dependency.
- The API MUST preserve the existing `wiki_embedding` table.
- Vault projection remains a post-commit derived write and is not included in this SQLite transaction.

## Acceptance

- `cargo test -p wiki-storage snapshot_outbox_and_embeddings_commit_together -- --nocapture`
- `cargo test -p wiki-storage snapshot_and_outbox_roll_back_when_embedding_insert_fails -- --nocapture`
- `cargo test -p wiki-cli --quiet`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
