# Requirements: Evidence Fixer Plan

## Behavior

- Add `wiki-cli governance fixer-plan --scan <scan.json>`.
- Input is the JSON artifact from `wiki-cli governance scan`.
- The command is dry-run only: it must not open or create `wiki.db`, write Vault
  projection, write `palace.db`, or emit outbox events.
- Output is a typed `EvidenceFixerPlan` with actions:
  - `promote_status`
  - `upgrade_source_reference`
  - `replace_deprecated_tag`
  - `correct_entry_type`
  - `merge_duplicate`
  - `semantic_patch`
  - `retire_page`
  - `add_missing_section`
  - `dedupe_source_reference`
  - `set_title_from_h1`
- Rule-determined fixes can be `ready` from scan evidence alone.
- Near-duplicate merge actions require either exact DB identity or web
  cross-verification. Without explicit web permission/configuration they are
  `blocked`.
- Semantic patches require old text, new text, target range, source chain, and a
  verifier-passed flag. Missing evidence or invalid JSON must not become an
  executable action.
- Retire actions must include `tombstone_required=true`.
- `--report-dir` writes sibling JSON and Markdown reports. JSON is the source of
  truth; Markdown is a rendered human view.

## Privacy

- Web verification is opt-in with `--allow-web-search`.
- Private viewer scopes do not send search queries unless
  `--allow-private-web-search` is also passed.
- `--internal-only` keeps all web-required actions blocked.

## Compatibility

- Existing `governance scan` behavior remains unchanged.
- Existing LLM/search smoke commands remain unchanged.
- No DB migration is required.

## Acceptance

- The command can build a plan from scan JSON without a DB file.
- Rule-determined actions become `ready`.
- Near duplicates are blocked without web cross-verification.
- Invalid semantic patch JSON fails the command.
- Semantic patches without source evidence are blocked.
- Report directory paths are vault-relative when `--wiki-dir` is set.
