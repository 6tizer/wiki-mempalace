# Requirements: Audit v2 PR 11 Vault Docs Hardening

## Scope

按 roadmap PR 11 收敛 Vault projection safety、docs consistency、slow hardening lane：

- `write_projection` 不能为全标点/空标题生成 `.md` 空 basename。
- YAML frontmatter 字符串必须转义双引号、反斜杠、换行、tab 和控制字符。
- 只有带显式 `managed_by: wiki-mempalace` + `managed_kind: wiki-page-projection`
  marker 的 `pages/` 文件才视为 engine-managed。
- stale / obsolete managed page 不直接删除，移入 `.wiki/trash/projection/`。
- 写入目标若已存在但没有 managed marker，projection 必须失败，不能覆盖手写页。
- 写入目标若属于另一个 managed projection page，先 quarantine 再写新页。
- `pages/` 或 `pages/{entry_type}/` 任一写入父目录不能是 symlink，canonical parent
  必须留在 canonical `pages/` 内。
- root projection 的 `index.md` / `log.md` 不能是 symlink，写入必须使用 guarded
  atomic replace。
- 手写 UUID page 没有 managed marker 时不得被 projection cleanup 误删。
- 文档中 architecture、README、roadmap、spec index 对当前状态不能互相矛盾。
- 新增 scheduled/manual hardening smoke lane，覆盖 perf、MCP malformed/boundary、DB corruption、
  CJK retrieval、bank/scope matrix。

## Acceptance

- `cargo test -p wiki-kernel wiki_writer -- --nocapture` 覆盖空 slug、frontmatter escape、
  managed marker、quarantine、手写 UUID 保留、未标记同名页不覆盖。
- `bash -n scripts/hardening-smoke.sh` 通过。
- `.github/workflows/hardening.yml` 提供 scheduled/manual slow lane。
- `docs/architecture.md` 不再包含重复章节或 Notion 增量同步未实现旧状态。
- `docs/roadmap.md` 将 PR 10/11 状态回填到当前事实。

## Non-goals

- 不运行生产 `/Users/mac-mini/Documents/wiki` 写操作。
- 不把 hardening lane 加进 required PR quick gate。
- 不做真实 Vault cleanup apply；本 PR 只改变未来 projection 的安全边界。
