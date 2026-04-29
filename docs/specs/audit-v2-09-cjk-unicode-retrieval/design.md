# Design: Audit v2 PR 09 CJK / Unicode Retrieval

## Current Problem

`rust-mempalace` 旧逻辑使用 ASCII-only token split：

- 中文 query 会被切成空 token。
- 空 token 被替换成 `"memory"`，可能命中无关内容。
- 一旦 `"memory"` 有结果，LIKE fallback 不会执行。
- rerank / sparse embedding 也用 ASCII lowercase，Unicode 信号弱。

## Approach

### Unicode Token Builder

新增 shared helpers：

- `unicode_tokens(query)`：按 `char::is_alphanumeric()` 保留 Unicode token，并用 Unicode lowercase。
- `build_fts_query(query) -> Option<String>`：有 token 时输出 quoted FTS phrase；无 token 时返回 `None`。
- `unicode_lower`：替代 ASCII-only lower。

FTS query 仍 quote 每个 token，防止用户输入 `OR` / FTS operator 变成语法。

### CJK Fallback

新增 `contains_cjk` 和 `char_ngrams`：

- CJK query 会生成 exact token、bigram、trigram LIKE patterns。
- FTS 无结果时走 fallback。
- CJK query 即使 FTS 有部分结果，也合并 LIKE fallback，补上 tokenizer 漏召回。
- fallback 继续应用 wing/hall/room/bank filters，并按 drawer id 去重。

### Ranking

`rerank_rows` 使用 `relevance_terms`：

- Unicode tokens。
- CJK bigram/trigram terms。

`sparse_embedding` 也保留 CJK token 和 `cjk:<bigram>` 特征，避免 vector rerank 对中文完全失明。

## Risk Controls

- 不把 LIKE fallback 扩到所有 ASCII query；只有 FTS empty 或 CJK query 才触发。
- LIKE patterns 上限 24，避免 query 过长导致 SQL 膨胀。
- fallback 使用 SQL parameters 和 LIKE escaping。
- e2e 使用 temp palace，不碰真实数据。
