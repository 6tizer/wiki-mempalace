# Design: Audit v2 PR 01 MCP Input Boundary

## Approach

- Add a shared `1..=100` MCP result limit policy in `wiki-cli` MCP handling.
- Add the same bounded helper to standalone `rust-mempalace` MCP handling.
- Keep service APIs unchanged; this PR only hardens MCP entrypoints.
- Validate lint report names before joining paths:
  - trim input;
  - reject absolute paths, `/`, `\`, `..`, empty names, hidden names;
  - allow slug characters `[A-Za-z0-9._-]`;
  - normalize to a single `.md` suffix.
- Reject `<wiki-root>/reports` if it is a symlink, then canonicalize it after
  creation and reject it if it resolves outside the wiki root.
- Write lint reports through a temp file created inside the canonical reports
  directory, then rename it over the final slug path so existing file symlinks
  are not followed.

## Error Shape

- `wiki-cli` MCP non-integer limits use existing typed `invalid_params`.
- `rust-mempalace` MCP non-integer limits use the existing JSON-RPC tool error
  path.
- Lint report path validation returns `io::ErrorKind::InvalidInput`.

## Compatibility

- Existing omitted limits keep defaults: `wiki_query=50`, mempalace
  search/reflect=`8`.
- Existing slug report names such as `lint-<timestamp>` continue to work.
