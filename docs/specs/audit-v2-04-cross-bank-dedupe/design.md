# Design: Audit v2 PR 04 Cross-Bank Drawer Dedupe

## Approach

- Replace the drawer uniqueness index:
  - new schema creates `idx_drawers_bank_hash` on `(bank_id, content_hash)`;
  - migration drops `idx_drawers_hash` and creates the new composite index.
- Keep `content_hash = sha256(source_path, content)` unchanged so same source
  payload still maps to the same hash across banks.
- Update insertion preflight queries:
  - `mine_path` uses `WHERE bank_id = ? AND content_hash = ?`;
  - `mine_path_convos` uses the same bank-scoped check;
  - `LiveMempalaceSink::insert_drawer` uses the sink bank in the duplicate
    check.
- Add tests at both layers:
  - DB migration allows same hash across banks and rejects same-bank duplicate;
  - `mine_path` can import the same file into two banks and remains idempotent
    per bank;
  - live sink can project the same page content into two banks and remains
    idempotent per bank.

## Compatibility

- Old DBs keep existing rows; the old global unique index is removed during
  additive migration.
- Existing callers that do not pass a bank continue to use `default`, so
  same-bank duplicate behavior is unchanged.
