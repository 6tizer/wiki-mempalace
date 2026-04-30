# Requirements: Web-backed Synthesis Composer

## Behavior

- Add `wiki-cli research-synthesis compose --candidate <id>`.
- Add `wiki-cli research-synthesis run`.
- Composer consumes a `SynthesisDiscoveryReport` candidate, internal wiki
  evidence, external web evidence, LLM draft, and verifier verdict.
- `compose` defaults to preflight. It writes a synthesis page only with
  `--apply`.
- `run` discovers current candidates and composes each candidate under the same
  guard rules.
- `--internal-only` disables web search and only permits internal citations.
- Private viewer scopes do not send web queries unless
  `--allow-private-web-search` is passed.
- Tests/offline runs can pass `--web-evidence`, `--draft-json`, and
  `--verifier-json` to exercise the same validation/write path without network
  or LLM calls.

## Evidence Rules

- Non-internal runs require cross-verified web evidence.
- Every key finding must cite an allowed `internal:<page_id>` or
  `web:<content_hash|url>` citation.
- Unknown citations, empty findings, missing drafts, missing verifier verdicts,
  and rejected verifier output block apply.
- The synthesis page is written as `entry_type=synthesis`,
  `status=in_review`, `confidence=high`.
- Page sections are fixed: 研究问题、综合分析、关键发现、来源列表、外部证据、行动建议.

## Acceptance

- Fake dual web evidence + fake LLM/verifier JSON can exercise full compose and
  apply in tests.
- Missing `--allow-private-web-search` blocks private-scope web research before
  any external call.
- Valid compose `--apply` writes a DB page through normal save/outbox/projection
  paths, never directly to `palace.db`.
- Invalid citations do not write pages.
