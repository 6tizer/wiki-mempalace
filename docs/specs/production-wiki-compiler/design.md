# Design: Production Wiki Compiler

## Summary

Create an internal `WikiCompilerRunner` and move `batch-ingest` onto it. The
runner compiles source Markdown into the Notion-equivalent output contract:
summary plus visible concept/entity pages with bidirectional references.

## Plain-Language Design

- Module role: compile raw source articles into human-visible wiki entries.
- Data it asks for: source article body/frontmatter, existing wiki pages,
  existing concept/entity names, LLM output.
- Data it returns: compile stats, generated/updated page ids, skipped reasons,
  and a run report.

## Data Model / Interfaces

### LLM Output

The compiler needs a richer plan than current `LlmIngestPlanV1` materializes.
The implementation can extend the existing plan or introduce a compiler-specific
plan type, but it must contain:

- summary:
  - title,
  - one-sentence summary,
  - 3-5 key insights,
  - confidence,
  - wiki tags,
  - personal note.
- concepts:
  - canonical name,
  - kind: `concept`,
  - definition,
  - key points,
  - tags,
  - related concept names.
- entities:
  - canonical name,
  - kind: `entity`,
  - entity category,
  - definition/profile,
  - key points,
  - tags,
  - related concept/entity names.
- source metadata:
  - author,
  - publisher,
  - published_at,
  - external URL.

### Compiler Runner

Internal runner responsibilities:

- scan candidate source files,
- select tiny sample or limited batch,
- call LLM,
- validate plan,
- deduplicate summary/concept/entity targets,
- write DB pages through `LlmWikiEngine`,
- write/update managed Vault pages through projection-compatible paths,
- update source `compiled_to_wiki`,
- append outbox through existing repository save path,
- emit a report.

### CLI Surface

Keep `batch-ingest` as the compatible entrypoint.

Add only the minimum flags needed for the production sample:

- `--origin <x|wechat|all>` to constrain source origin.
- `--scope <SCOPE>` to avoid hardcoded `private:batch-ingest` for production.
- `--source-id <UUID>` or `--source-path <PATH>` for exact sample selection.
- Existing `--limit`, `--dry-run`, and `--delay-secs` remain.

Default production command shape:

```bash
cargo run -p wiki-cli -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  --wiki-dir /Users/mac-mini/Documents/wiki --sync-wiki \
  --viewer-scope shared:wiki \
  batch-ingest \
  --vault /Users/mac-mini/Documents/wiki \
  --scope shared:wiki \
  --origin x \
  --limit 1
```

## Flow

```mermaid
flowchart TD
    A["Scan sources with compiled_to_wiki=false"] --> B["Select sample or batch"]
    B --> C["Read source body and frontmatter"]
    C --> D["LLM compiler plan"]
    D --> E["Validate tags, page types, and references"]
    E --> F["Dedup summary by source identity"]
    F --> G["Create summary page"]
    G --> H["Dedup concepts/entities by canonical name"]
    H --> I["Create or update concept/entity pages"]
    I --> J["Add summary -> concept/entity links"]
    J --> K["Add concept/entity -> summary source refs"]
    K --> L["Mark source compiled locally"]
    L --> M["Save DB + outbox"]
    M --> N["Project Vault pages"]
    N --> O["Consume outbox to Mempalace"]
    O --> P["Run query/explain smoke"]
    P --> Q["Write run report"]
```

## Page Contracts

### Summary

- Path: `pages/summary/摘要：{source title}.md`
- Status: `approved`
- Required sections:
  - `## 一句话摘要`
  - `## 关键洞察`
  - `## 提取的概念`
  - `## 原始文章信息`
  - `## 个人评注`

### Concept

- Path: `pages/concept/{canonical name}.md`
- Status: `draft`
- Required sections:
  - `## 定义`
  - `## 关键要点`
  - `## 来源引用`
  - `## 关联概念`

### Entity

- Path: `pages/entity/{canonical name}.md`
- Status: `draft`
- Required sections:
  - `## 概述`
  - `## 关键信息`
  - `## 来源引用`
  - `## 关联概念`

## Dedup Rules

- Summary:
  - first match by source id,
  - then Notion UUID,
  - then external article URL,
  - title match is last fallback only.
- Concept/entity:
  - normalize title by trimming, lowercasing ASCII, folding punctuation and
    repeated whitespace,
  - exact normalized match wins,
  - conservative fuzzy match can update an existing page only when one clear
    candidate exists,
  - multiple fuzzy candidates stop the source and require review.
- Source references:
  - dedup by summary page id first,
  - then source id,
  - then external article URL.

## Edge Cases

- Short or empty source body: create low-confidence summary only, skip
  concept/entity extraction.
- LLM returns no useful concepts: create summary, mark report warning, do not
  fail the whole run.
- LLM returns generic concepts such as `AI` or `效率`: reject them from
  concept/entity page creation and record a warning.
- Existing concept/entity has no `## 来源引用`: append the section instead of
  rewriting the whole page.
- Existing page has duplicate source references: keep one canonical reference
  and do not append another.
- Mempalace consume fails after DB/Vault write: stop before scale-up and record
  recovery commands.

## Compatibility

- Existing `batch-ingest --dry-run` must continue scanning sources safely.
- Existing automation job name remains `batch-ingest`.
- Existing source projection and `write_projection` ownership stay unchanged:
  sources are maintained by source projection; pages are maintained by page
  projection/compiler.
- Existing `LlmIngestPlanV1` tests may be kept, but production compiler tests
  must cover concept/entity materialization.

## Spec Sync Rules

- If implementation needs a different CLI flag, update this file before code.
- If review changes module boundaries, update flow and tasks before continuing.
- If production sample reveals the Notion instructions require a new page type,
  update PRD first.

## Test Strategy

- Unit:
  - compiler plan parsing,
  - title normalization,
  - summary dedup,
  - concept/entity dedup,
  - source reference dedup,
  - generic concept rejection.
- Integration:
  - one fake source creates summary + concept + entity pages,
  - existing concept receives one new source reference,
  - duplicate summary source is skipped,
  - `batch-ingest --dry-run` remains read-only.
- Production smoke:
  - backup production vault,
  - compile one X source and one WeChat source,
  - consume outbox to Mempalace,
  - run `query` and `query/explain --palace-db`,
  - write run report.
