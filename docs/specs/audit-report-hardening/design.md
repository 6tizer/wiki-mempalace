# Design: Audit Report Hardening

## MCP Input Cap

Replace `read_line()` with a small `BufRead::fill_buf()` loop that copies into a bounded buffer and errors once the line would exceed 10 MiB.

## LLM Plan Validation

Add `LlmIngestPlanV1::validate_bounds()` in `wiki-core` so both CLI and MCP callers share the same contract. The validation is intentionally structural and conservative: count limits, text length limits, and version gate only. Tag policy remains handled by existing schema validation.

## Redaction

Keep current line-level replacement behavior to avoid partial secret leakage. Add simple deterministic detectors without new dependencies.

## Outbox Batch Flush

Extend `WikiRepository` with `append_outbox_batch()`. Default trait impl loops for non-SQLite/test implementations; `SqliteRepository` overrides with a transaction. Engine flush trims only whole batches known to have committed.

## FTS Query

Continue splitting to ASCII alphanumeric tokens, then quote each token. This preserves current broad matching while preventing raw operator syntax from user text.

## Deferred Architecture Items

The audit report's major scale items remain outside this patch:

- row-level state storage
- ANN-backed embedding retrieval
- CLI command module split
- MCP API reference generation

