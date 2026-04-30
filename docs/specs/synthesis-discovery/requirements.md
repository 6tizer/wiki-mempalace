# Requirements: Synthesis Discovery

## Behavior

- Add `wiki-cli research-synthesis discover`.
- The command is read-only: it must not mutate `wiki.db`, Vault projection, or
  `palace.db`.
- With `--scan <scan.json>`, discovery uses an existing governance scan and must
  not open/create a DB.
- Without `--scan`, discovery runs an in-memory governance scan for the active
  `--viewer-scope`, then derives candidates from that scan.
- Output is a typed JSON report; Markdown is a sibling human view when
  `--report-dir` is provided.
- Report directories are vault-relative when `--wiki-dir` is set.

## Candidate Rules

- Single tag: at least 10 concept/entity pages share the same tag.
- Double tag: two tags share at least 3 concept/entity pages.
- Triple tag: each tag has at least 5 concept/entity pages, and every pairwise
  edge has at least 2 shared concept/entity pages. This is a triangle rule, not
  a triple-intersection rule.
- Quad tag: a valid triple anchor exists from prior synthesis coverage, then a
  fourth tag with at least 5 pages is introduced only when it is distant from
  every anchor tag (`pairwise intersection <= 3`).
- Prior synthesis coverage suppresses duplicate candidates with the same tag
  set.
- Deprecated tags do not participate because governance scan excludes them from
  synthesis tag signals.

## Acceptance

- JSON output includes deterministic candidate IDs, kind, tags, anchor tags,
  page evidence, source domains, score, and rationale.
- Ranking prefers quad > triple > double > single, then source diversity, then
  page evidence count.
- Per run defaults: at most 1 single/double candidate, 1 triple candidate, and 1
  quad candidate.
- Focused tests cover single/double thresholds, triple triangle behavior, quad
  distant-tag behavior, coverage dedupe, report writing, and `--scan` no-DB
  mode.
