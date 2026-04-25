# Design: Vault Report Paths

## Path Resolution

Add small helpers in `wiki-cli`:

- `resolve_wiki_relative_path(wiki_root, path)`:
  - absolute path: return unchanged.
  - relative path with `wiki_root`: return `wiki_root.join(path)`.
  - relative path without `wiki_root`: return unchanged.
- `default_dashboard_output(wiki_root)`:
  - with `wiki_root`: `wiki_root/reports/dashboard.html`.
  - without `wiki_root`: `wiki/reports/dashboard.html`.
- `default_suggest_report_dir(wiki_root)`:
  - with `wiki_root`: `wiki_root/reports/suggestions`.
  - without `wiki_root`: `wiki/reports/suggestions`.

## CLI Shape

- Change `dashboard --output` from required-by-default `PathBuf` to
  `Option<PathBuf>` so the default can depend on `--wiki-dir`.
- Change `suggest --report-dir` from `Option<PathBuf>` to
  `Option<Option<PathBuf>>` so the command can distinguish:
  - flag absent: no files.
  - flag present without value: default report dir.
  - flag present with value: supplied report dir.

## Compatibility

- Existing no-`--wiki-dir` defaults keep the historical `wiki/reports/...`
  locations.
- Existing absolute paths remain exact.
- Existing explicit relative paths without `--wiki-dir` remain cwd-relative.

## Docs

Update `AGENTS.md` so future Agents can rely on the code rule: with
`--wiki-dir`, report output relative paths are vault-relative.
