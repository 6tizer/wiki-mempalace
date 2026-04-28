# Design: Reliability Test Matrix

## Matrix

| Risk | Coverage |
| --- | --- |
| Partial outbox batch commit | New SQLite trigger failure test proves `append_outbox_batch()` rolls back all events. |
| Large snapshot regression | New storage smoke saves and reloads thousands of claims/pages in a temp DB. |
| MCP oversized request | Existing `read_line_limited_rejects_oversized_line`. |
| MCP malformed request | New parser helper test maps malformed JSON to `parse_error`. |
| LLM bad JSON | New `parse_json_object_slice` tests cover fenced JSON and malformed fallback without network calls. |
| Concurrent writer / busy DB | PR #64 writer lease tests cover busy fail-fast; this PR references them rather than duplicating. |

## Implementation Notes

- Keep all new tests deterministic and tempdir-backed.
- Use SQLite triggers for failure injection instead of corrupting files.
- Only refactor MCP parse logic enough to make malformed request behavior directly testable.
