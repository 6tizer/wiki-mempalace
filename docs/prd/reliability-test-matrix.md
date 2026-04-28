# PRD: Reliability Test Matrix

## Status

Implemented in PR #65.

## Goal

Add focused regression coverage for reliability risks called out by the audit roadmap without changing product behavior.

## Scope

- DB transaction failure injection.
- Large snapshot smoke coverage.
- MCP malformed/oversized request coverage.
- LLM malformed JSON extraction coverage.
- Documentation that maps the matrix to existing and new tests.

## Out of Scope

- New runtime behavior.
- New fuzzing framework.
- Slow nightly performance benchmarks.
- Production DB/Vault mutation.

## Success Criteria

- New tests fail on partial batch commit regressions.
- Large snapshot roundtrip remains covered in normal workspace tests.
- MCP malformed JSON returns typed parse errors.
- LLM malformed JSON handling is covered without network calls.
