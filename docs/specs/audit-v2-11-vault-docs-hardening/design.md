# Design: Audit v2 PR 11 Vault Docs Hardening

## Projection Ownership

旧逻辑把 `pages/` 下任何带合法 UUID `id:` 的 Markdown 都当成 managed page。
这会误删手写页：人工页面也可能使用 UUID frontmatter。

新合同：

- engine projection 在 frontmatter 写入：
  - `managed_by: wiki-mempalace`
  - `managed_kind: wiki-page-projection`
- cleanup 只处理同时包含上述 marker 和合法 UUID `id:` 的文件。
- stale / obsolete managed file 通过 `fs::rename` 移入
  `.wiki/trash/projection/{subdir}__{filename}`，不做不可恢复删除。
- targeted projection `write_projection_pages` 仍只写选中 pages，不做 cleanup。

旧的无 marker 投影文件不会被自动删除；这是保守兼容选择，避免把历史手写页误判为投影页。

## Write Collision Guard

full projection 和 targeted projection 在写入目标路径前先检查现有文件：

- 路径不存在：允许写入。
- 现有文件是同一 page id 的 managed projection：允许原子替换。
- 现有文件是其他 page id 的 managed projection：先移动到 `.wiki/trash/projection/`，
  再写新 projection。
- 现有文件没有 managed marker、不是 UTF-8、不是普通文件或是 symlink：返回错误，
  不覆盖。
- 写入父目录必须 lexically/canonically 保持在 `pages/` 内，且 `pages/` 到 entry-type
  子目录的任一 ancestor 都不能是 symlink；这避免目标文件不存在时通过 symlinked
  directory 写出 vault。

这把“清理不误删”和“写入不覆盖手写页”分开处理。

Root projection outputs (`index.md` / `log.md`) 也走 guarded atomic writer：
parent 必须是 canonical wiki root，已存在目标必须是普通非 symlink 文件。

## Filename Fallback

`vault_page_filename(title)` 仍保留中文、折叠空白/标点、最长 80 字符。
新增 `projection_page_basename(page)`：

- slug 非空时使用原 slug。
- slug 为空时使用 `page-{page_id_prefix8}`。
- duplicate title disambiguation 仍在同一 `entry_type` 子目录内按 basename 计数。

## YAML Escaping

frontmatter 仍使用双引号字符串，但 `yaml_escape` 扩展为：

- `\\` -> `\\\\`
- `"` -> `\"`
- newline / carriage return / tab -> `\n` / `\r` / `\t`
- 其他控制字符 -> YAML double-quoted escape (`\xNN` / `\uNNNN` / `\UNNNNNNNN`)

## Hardening Lane

新增 `scripts/hardening-smoke.sh` 和 `.github/workflows/hardening.yml`。
workflow 只在 schedule/manual 触发，不进入 required PR quick gate。

Lane matrix：

- `perf`: large snapshot roundtrip smoke。
- `mcp-boundary`: MCP parser/schema/malformed/scope tests。
- `db-corruption`: intentionally invalid SQLite file must fail automation health。
- `cjk-retrieval`: CJK e2e and focused retrieval unit tests。
- `bank-scope`: wiki MCP bank derivation, rust-mempalace bank isolation, bridge scope filtering。

## Compatibility

- 新投影文件包含 managed marker；旧 consumers 读取 Markdown 不受影响。
- 旧无 marker files 被保留，不再作为 cleanup 对象。
- `.wiki/trash/projection/` 是恢复缓冲区，不进入 active graph。
- 如果历史无 marker projection 文件与新 DB page 发生同名冲突，projection 会失败；
  需要后续独立 dry-run cleanup spec 来迁移历史文件。
