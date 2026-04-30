# Requirements: Wiki Governance Scan

## Behavior

- Add `wiki-cli governance scan`.
- The scan is read-only: it must not save `wiki.db`, write Vault projection, write
  `palace.db`, or emit outbox events.
- The scan reports:
  - lifecycle promotion readiness and blockers;
  - reference issues: missing source, raw URL references, repeated source refs,
    missing source IDs, broken wikilinks;
  - existing lint findings;
  - existing gap findings;
  - exact and near duplicate groups;
  - retire candidates that are safe for later fixer planning;
  - tag usage and tag intersection signals for future synthesis discovery.
- The scan respects `--viewer-scope`.
- `--report-dir` writes sibling JSON and Markdown reports. JSON is the source of
  truth; Markdown is a rendered human view.

## Compatibility

- Existing `lint`, `gap`, `metrics`, `suggest`, and automation commands keep their
  behavior.
- Existing `wiki-cli lint` remains the write/report-producing lint command. The
  new governance scan does not replace it.
- No schema migration is required.

## Acceptance

- `governance scan` can run on an empty or small repo without panic.
- Hidden private-scope content does not appear in another viewer's scan.
- JSON and Markdown reports are generated from the same report object.
- Duplicate source URLs and repeated claim source IDs are detected.
- Deprecated tags are excluded from synthesis tag signals but reported as
  deprecated usage.
