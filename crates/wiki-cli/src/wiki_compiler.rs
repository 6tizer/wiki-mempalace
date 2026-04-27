use crate::llm;
use crate::{parse_scope, timestamp_slug};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use time::format_description::well_known::Rfc3339;
use wiki_core::llm_ingest_plan::LlmConceptDraft;
use wiki_core::{
    normalize_and_validate_tag_groups, parse_memory_tier, Confidence, DomainSchema, Entity,
    EntityId, EntityKind, EntryType, LlmEntityDraft, LlmIngestPlanV1, MemoryTier, PageContract,
    PageId, RawArtifact, RelationKind, Scope, SourceId, TypedEdge, WikiPage,
};
use wiki_kernel::{
    format_claim_doc_id, initial_status_for, write_projection, LlmWikiEngine, NoopWikiHook,
};
use wiki_storage::SqliteRepository;

pub(crate) struct BatchIngestOptions<'a> {
    pub(crate) vault: &'a Path,
    pub(crate) limit: Option<usize>,
    pub(crate) dry_run: bool,
    pub(crate) delay_secs: u64,
    pub(crate) sync_wiki: bool,
    pub(crate) wiki_root: Option<&'a Path>,
    pub(crate) scope: Option<&'a str>,
    pub(crate) origin: Option<&'a str>,
    pub(crate) source_path: Option<&'a Path>,
}

/// batch 单条写入 summary / 引擎时携带的 vault 元数据。
pub(crate) struct BatchIngestContext {
    pub(crate) source_title: String,
    pub(crate) source_url: String,
    pub(crate) source_tags: Vec<String>,
}

#[derive(Debug, Clone)]
struct SourceEntry {
    path: PathBuf,
    origin: Option<String>,
    source_id: Option<SourceId>,
    title: String,
    url: String,
    body: String,
    source_tags: Vec<String>,
}

#[derive(Debug, Default)]
struct PageMaterializationStats {
    summary_created: bool,
    concepts_created: usize,
    concepts_updated: usize,
    entities_created: usize,
    entities_updated: usize,
    warnings: Vec<String>,
}

/// 单条 source 编译结果。
struct IngestOneStats {
    claims: usize,
    entities: usize,
    relationships: usize,
    source_id: SourceId,
    page_stats: PageMaterializationStats,
}

pub(crate) fn default_vault_path() -> PathBuf {
    if let Ok(v) = std::env::var("WIKI_VAULT_DIR") {
        return PathBuf::from(v);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join("Documents").join("wiki")
}

pub(crate) fn batch_source_tags_for_ingest(batch: &BatchIngestContext) -> &[String] {
    &batch.source_tags
}

pub(crate) fn preflight_llm_plan_tags(
    plan: &LlmIngestPlanV1,
    source_tags: &[String],
    schema: &DomainSchema,
) -> Result<(), wiki_core::TagPolicyError> {
    let mut groups =
        Vec::with_capacity(plan.claims.len() + plan.concepts.len() + plan.entities.len() + 3);
    groups.push(source_tags);
    groups.push(plan.tags.as_slice());
    groups.push(plan.summary.tags.as_slice());
    groups.extend(plan.claims.iter().map(|claim| claim.tags.as_slice()));
    groups.extend(plan.concepts.iter().map(|concept| concept.tags.as_slice()));
    groups.extend(plan.entities.iter().map(|entity| entity.tags.as_slice()));
    normalize_and_validate_tag_groups(&groups, schema)?;
    Ok(())
}

pub(crate) fn batch_ingest_cmd(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    llm_config_path: &Path,
    vectors: bool,
    schema: &DomainSchema,
    heartbeat: &crate::AutomationHeartbeat<'_>,
    opts: BatchIngestOptions<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    heartbeat.tick();
    eprintln!("扫描未编译 source...");
    let mut sources = scan_uncompiled_sources(opts.vault, opts.origin, opts.source_path)?;
    eprintln!("  → 找到 {} 条未编译 source", sources.len());

    if let Some(n) = opts.limit {
        sources.truncate(n);
        eprintln!("  → --limit {}，处理前 {} 条", n, sources.len());
    }

    if opts.dry_run {
        for (i, s) in sources.iter().enumerate() {
            println!(
                "{}/{}) {} [{}] ({} 字符)",
                i + 1,
                sources.len(),
                s.title,
                s.origin.as_deref().unwrap_or("unknown"),
                s.body.len()
            );
        }
        return Ok(());
    }

    if sources.is_empty() {
        eprintln!("  → nothing to compile, done.");
        return Ok(());
    }

    let cfg = llm::load_llm_config(llm_config_path)?;
    let scope = parse_scope(opts.scope.unwrap_or("shared:wiki"));
    let mut runner = WikiCompilerRunner {
        eng,
        repo,
        cfg: &cfg,
        vectors,
        llm_config_path,
        schema,
        scope,
    };

    let mut compiled_paths: Vec<(PathBuf, SourceId)> = Vec::new();
    let mut ok_count = 0usize;
    let mut err_count = 0usize;

    for (i, src) in sources.iter().enumerate() {
        heartbeat.tick();
        eprintln!("[{}/{}] {}...", i + 1, sources.len(), src.title);

        match runner.compile_source(src) {
            Ok(stats) => {
                eprintln!(
                    "  ✓ claims={} entities={} rels={} source={} summary={} concepts={}/{} entities={}/{}",
                    stats.claims,
                    stats.entities,
                    stats.relationships,
                    stats.source_id.0,
                    if stats.page_stats.summary_created { "created" } else { "dedup" },
                    stats.page_stats.concepts_created,
                    stats.page_stats.concepts_updated,
                    stats.page_stats.entities_created,
                    stats.page_stats.entities_updated,
                );
                for warning in &stats.page_stats.warnings {
                    eprintln!("  ! {warning}");
                }
                compiled_paths.push((src.path.clone(), stats.source_id));
                ok_count += 1;
            }
            Err(e) => {
                eprintln!("  ✗ 失败：{e}");
                err_count += 1;
            }
        }

        if i + 1 < sources.len() && opts.delay_secs > 0 {
            std::thread::sleep(std::time::Duration::from_secs(opts.delay_secs));
        }
    }

    if opts.sync_wiki {
        if let Some(root) = opts.wiki_root {
            let stats = write_projection(root, &runner.eng.store, &runner.eng.audits)?;
            println!(
                "projection pages={} claims={} sources={}",
                stats.pages_written, stats.claims_written, stats.sources_written
            );
        }
    }
    write_run_report(opts.wiki_root.or(Some(opts.vault)), ok_count, err_count)?;
    for (path, source_id) in compiled_paths {
        mark_source_compiled(&path, source_id)?;
    }
    eprintln!("\n完成：成功={ok_count} 失败={err_count}");
    Ok(())
}

struct WikiCompilerRunner<'a> {
    eng: &'a mut LlmWikiEngine<NoopWikiHook>,
    repo: &'a SqliteRepository,
    cfg: &'a llm::LlmConfig,
    vectors: bool,
    llm_config_path: &'a Path,
    schema: &'a DomainSchema,
    scope: Scope,
}

impl WikiCompilerRunner<'_> {
    fn compile_source(
        &mut self,
        src: &SourceEntry,
    ) -> Result<IngestOneStats, Box<dyn std::error::Error>> {
        let uri = if src.url.is_empty() {
            format!("file://{}", src.path.display())
        } else {
            src.url.clone()
        };
        let batch = BatchIngestContext {
            source_title: src.title.clone(),
            source_url: src.url.clone(),
            source_tags: src.source_tags.clone(),
        };

        let user = format!("Source URI:\n{uri}\n\nBody:\n{}", src.body);
        let reply = llm::complete_chat(self.cfg, llm::ingest_llm_system_prompt(), &user, 8192)?;
        let slice = llm::parse_json_object_slice(&reply);
        let plan: LlmIngestPlanV1 = serde_json::from_str(slice)
            .map_err(|e| format!("JSON parse error: {e}; raw={reply}"))?;
        preflight_llm_plan_tags(&plan, batch_source_tags_for_ingest(&batch), self.schema)?;

        let sid = match self.existing_source_id(src.source_id, &uri) {
            Some(source_id) => source_id,
            None => self.ingest_source(&uri, &src.body, &batch)?,
        };
        write_source_id(&src.path, sid)?;

        if self.vectors {
            let app = llm::load_app_config(self.llm_config_path)?;
            let body_short = truncate_chars(&src.body, 16000);
            let vec = llm::embed_first(&app, &body_short)?;
            self.repo
                .upsert_embedding(&format!("source:{}", sid.0), &vec)?;
        }

        for c in &plan.claims {
            let tier = parse_memory_tier(&c.tier).unwrap_or(MemoryTier::Semantic);
            let cid = self.eng.file_claim_with_tags(
                c.text.clone(),
                self.scope.clone(),
                tier,
                "batch-ingest",
                c.tags.iter().map(String::as_str),
            )?;
            self.eng.attach_sources(cid, &[sid])?;
            self.eng
                .save_to_repo_and_flush_outbox_with_policy(self.repo, 128, 3)?;
            if self.vectors {
                let app = llm::load_app_config(self.llm_config_path)?;
                let vec = llm::embed_first(&app, &c.text)?;
                self.repo
                    .upsert_embedding(&format_claim_doc_id(cid), &vec)?;
            }
        }

        for ed in &plan.entities {
            let kind = EntityKind::parse(&ed.kind);
            let entity = Entity {
                id: EntityId(uuid::Uuid::new_v4()),
                kind,
                label: ed.label.clone(),
                scope: self.scope.clone(),
            };
            let _ = self.eng.add_entity(entity);
        }

        for rd in &plan.relationships {
            let from_id =
                find_entity_id_by_label(&self.eng.store.entities, &rd.from_label, &self.scope);
            let to_id =
                find_entity_id_by_label(&self.eng.store.entities, &rd.to_label, &self.scope);
            if let (Some(from), Some(to)) = (from_id, to_id) {
                let edge = TypedEdge {
                    from,
                    to,
                    relation: RelationKind::parse(&rd.relation),
                    confidence: 0.7,
                    source_ids: vec![sid],
                };
                let _ = self.eng.add_edge(edge);
            }
        }

        let page_stats =
            materialize_compiler_pages(self.eng, &plan, &batch, &uri, &self.scope, self.schema);
        self.eng
            .save_to_repo_and_flush_outbox_with_policy(self.repo, 128, 3)?;

        Ok(IngestOneStats {
            claims: plan.claims.len(),
            entities: plan.entities.len(),
            relationships: plan.relationships.len(),
            source_id: sid,
            page_stats,
        })
    }

    fn ingest_source(
        &mut self,
        uri: &str,
        body: &str,
        batch: &BatchIngestContext,
    ) -> Result<SourceId, Box<dyn std::error::Error>> {
        let sid = self.eng.ingest_raw_with_tags(
            uri,
            body,
            self.scope.clone(),
            "batch-ingest",
            batch_source_tags_for_ingest(batch),
        )?;
        self.eng
            .save_to_repo_and_flush_outbox_with_policy(self.repo, 128, 3)?;
        Ok(sid)
    }

    fn existing_source_id(&self, frontmatter_id: Option<SourceId>, uri: &str) -> Option<SourceId> {
        if let Some(source_id) = frontmatter_id {
            if self.eng.store.sources.contains_key(&source_id) {
                return Some(source_id);
            }
        }
        find_existing_source_id_by_uri(&self.eng.store.sources, uri, &self.scope)
    }
}

fn find_existing_source_id_by_uri(
    sources: &HashMap<SourceId, RawArtifact>,
    uri: &str,
    scope: &Scope,
) -> Option<SourceId> {
    sources
        .values()
        .find(|source| source.scope == *scope && source.uri == uri)
        .map(|source| source.id)
}

fn scan_uncompiled_sources(
    vault: &Path,
    origin_filter: Option<&str>,
    source_path: Option<&Path>,
) -> Result<Vec<SourceEntry>, Box<dyn std::error::Error>> {
    let sources_dir = vault.join("sources");
    if !sources_dir.exists() {
        return Err(format!("sources 目录不存在：{}", sources_dir.display()).into());
    }
    let re_fm = regex::Regex::new(r"(?s)^---\s*\n(.*?)\n---\s*\n")?;
    let mut entries = Vec::new();

    for dent in walkdir::WalkDir::new(&sources_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = dent.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        if let Some(expected) = source_path {
            let expected = if expected.is_absolute() {
                expected.to_path_buf()
            } else {
                vault.join(expected)
            };
            if path != expected {
                continue;
            }
        }
        let origin = path
            .strip_prefix(&sources_dir)
            .ok()
            .and_then(|p| p.components().next())
            .map(|c| c.as_os_str().to_string_lossy().into_owned());
        if let Some(filter) = origin_filter {
            if filter != "all" && origin.as_deref() != Some(filter) {
                continue;
            }
        }

        let content = std::fs::read_to_string(path)?;
        let fm_caps = re_fm.captures(&content);
        let fm_text = fm_caps
            .as_ref()
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("");
        let fm = parse_frontmatter_kv(fm_text);

        if fm.get("compiled_to_wiki").map(|v| v.as_str()) != Some("false") {
            continue;
        }

        let title = fm.get("title").cloned().unwrap_or_default();
        let url = fm.get("url").cloned().unwrap_or_default();
        let source_id = parse_source_id(fm.get("source_id"));
        let source_tags = parse_frontmatter_tags(fm_text, "tags");
        let body = if let Some(caps) = fm_caps {
            content[caps.get(0).unwrap().end()..].trim().to_string()
        } else {
            content.trim().to_string()
        };

        if body.len() < 50 {
            eprintln!("  跳过（正文过短）：{}", title);
            continue;
        }

        entries.push(SourceEntry {
            path: path.to_path_buf(),
            origin,
            source_id,
            title,
            url,
            body,
            source_tags,
        });
    }

    entries.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(entries)
}

fn parse_source_id(raw: Option<&String>) -> Option<SourceId> {
    raw.and_then(|s| uuid::Uuid::parse_str(s.trim()).ok())
        .map(SourceId)
}

fn materialize_compiler_pages(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    plan: &LlmIngestPlanV1,
    batch: &BatchIngestContext,
    uri: &str,
    scope: &Scope,
    schema: &DomainSchema,
) -> PageMaterializationStats {
    let mut stats = PageMaterializationStats::default();
    let summary_title = format!("摘要：{}", batch.source_title);
    let source_ref = SourceReference {
        summary_title: summary_title.clone(),
        source_url: if batch.source_url.trim().is_empty() {
            uri.to_string()
        } else {
            batch.source_url.clone()
        },
    };

    let entity_links = extracted_entity_links(plan);
    if plan.should_materialize_summary_page()
        && find_summary_page(
            &eng.store.pages,
            &summary_title,
            &source_ref.source_url,
            scope,
        )
        .is_none()
    {
        let md = render_summary_markdown(plan, &source_ref.source_url, &entity_links);
        let status = initial_status_for(Some(&EntryType::Summary), schema);
        let mut page = WikiPage::new(summary_title.clone(), md, scope.clone())
            .with_entry_type(EntryType::Summary)
            .with_status(status);
        page.confidence = parse_plan_confidence(plan.normalized_summary_confidence());
        page.tags = if plan.summary.tags.is_empty() {
            plan.tags.clone()
        } else {
            plan.summary.tags.clone()
        };
        page.source_url = Some(source_ref.source_url.clone());
        page.source_tags = batch.source_tags.clone();
        page.compiled_by = Some("batch-ingest".to_string());
        page.last_compiled_at = Some(time::OffsetDateTime::now_utc());
        eng.write_page(page, "batch-ingest");
        stats.summary_created = true;
    }

    for concept in &plan.concepts {
        let name = concept.canonical_name.trim();
        if name.is_empty() || is_generic_concept(name) {
            stats
                .warnings
                .push(format!("skip generic entity/concept: {name}"));
            continue;
        }
        let normalized = normalize_title_for_dedup(name);
        if let Some(pid) =
            find_page_by_normalized_title(&eng.store.pages, &EntryType::Concept, &normalized, scope)
        {
            let mut updated_page = None;
            if let Some(page) = eng.store.pages.get_mut(&pid) {
                if append_source_reference(page, &source_ref) {
                    updated_page = Some(page.clone());
                }
            }
            if let Some(page) = updated_page {
                eng.write_page(page, "batch-ingest");
                stats.concepts_updated += 1;
            }
            continue;
        }

        let page = build_concept_page(concept, &source_ref, scope, schema);
        eng.write_page(page, "batch-ingest");
        stats.concepts_created += 1;
    }

    for ed in &plan.entities {
        let Some(kind) = compiler_page_kind(ed) else {
            stats
                .warnings
                .push(format!("skip generic entity/concept: {}", ed.label));
            continue;
        };
        let normalized = normalize_title_for_dedup(ed.canonical_or_label());
        if let Some(pid) =
            find_page_by_normalized_title(&eng.store.pages, &kind, &normalized, scope)
        {
            let mut updated_page = None;
            if let Some(page) = eng.store.pages.get_mut(&pid) {
                if append_source_reference(page, &source_ref) {
                    updated_page = Some(page.clone());
                }
            }
            if let Some(page) = updated_page {
                eng.write_page(page, "batch-ingest");
                match kind {
                    EntryType::Concept => stats.concepts_updated += 1,
                    EntryType::Entity => stats.entities_updated += 1,
                    _ => {}
                }
            }
            continue;
        }

        let page = build_concept_entity_page(ed, kind.clone(), &source_ref, scope, schema);
        eng.write_page(page, "batch-ingest");
        match kind {
            EntryType::Concept => stats.concepts_created += 1,
            EntryType::Entity => stats.entities_created += 1,
            _ => {}
        }
    }
    stats
}

fn build_concept_page(
    concept: &LlmConceptDraft,
    source_ref: &SourceReference,
    scope: &Scope,
    schema: &DomainSchema,
) -> WikiPage {
    let title = concept.canonical_name.trim();
    let definition = if concept.definition.trim().is_empty() {
        "（待后续 compiler plan 字段补全）".to_string()
    } else {
        concept.definition.trim().to_string()
    };
    let status = initial_status_for(Some(&EntryType::Concept), schema);
    PageContract::new(title, EntryType::Concept)
        .with_source("batch-ingest")
        .with_tags(concept.tags.clone())
        .with_section("定义", definition)
        .with_section("关键要点", render_bullets(&concept.key_points))
        .with_section("本文语境", render_wikilinks(&concept.related_names))
        .with_section("来源引用", source_reference_line(source_ref))
        .into_page(scope.clone(), status)
}

fn build_concept_entity_page(
    ed: &LlmEntityDraft,
    kind: EntryType,
    source_ref: &SourceReference,
    scope: &Scope,
    schema: &DomainSchema,
) -> WikiPage {
    let title = ed.canonical_or_label().trim();
    let definition = if !ed.profile_or_definition().trim().is_empty() {
        ed.profile_or_definition().trim().to_string()
    } else if kind == EntryType::Concept {
        format!("{title} 是从本 source 抽取的概念；当前 LlmIngestPlanV1 尚未提供 definition/key_points 字段。")
    } else {
        format!("{title} 是从本 source 抽取的实体；当前 LlmIngestPlanV1 尚未提供 profile/key_points 字段。")
    };
    let status = initial_status_for(Some(&kind), schema);
    PageContract::new(title, kind)
        .with_source("batch-ingest")
        .with_tags(ed.tags.clone())
        .with_section("定义", definition)
        .with_section("关键要点", render_bullets(&ed.key_points))
        .with_section("本文语境", render_wikilinks(&ed.related_names))
        .with_section("来源引用", source_reference_line(source_ref))
        .into_page(scope.clone(), status)
}

fn render_bullets(items: &[String]) -> String {
    let lines: Vec<_> = items
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| format!("- {s}"))
        .collect();
    if lines.is_empty() {
        "（暂无）".to_string()
    } else {
        lines.join("\n")
    }
}

fn render_wikilinks(names: &[String]) -> String {
    let lines: Vec<_> = names
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| format!("- [[{s}]]"))
        .collect();
    if lines.is_empty() {
        "（暂无）".to_string()
    } else {
        lines.join("\n")
    }
}

fn render_summary_markdown(
    plan: &LlmIngestPlanV1,
    foot_url: &str,
    entity_links: &[String],
) -> String {
    let mut md = plan.to_five_section_summary_body(Some(foot_url));
    let links = if entity_links.is_empty() {
        "（暂无）".to_string()
    } else {
        entity_links
            .iter()
            .map(|name| format!("- [[{name}]]"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    md = replace_section_body(&md, "提取的概念", &links).unwrap_or(md);
    md
}

fn extracted_entity_links(plan: &LlmIngestPlanV1) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for concept in &plan.concepts {
        if !concept.canonical_name.trim().is_empty()
            && !is_generic_concept(&concept.canonical_name)
            && seen.insert(normalize_title_for_dedup(&concept.canonical_name))
        {
            out.push(concept.canonical_name.trim().to_string());
        }
    }
    for ed in &plan.entities {
        if compiler_page_kind(ed).is_some() {
            let key = normalize_title_for_dedup(ed.canonical_or_label());
            if seen.insert(key) {
                out.push(ed.canonical_or_label().trim().to_string());
            }
        }
    }
    out
}

fn compiler_page_kind(ed: &LlmEntityDraft) -> Option<EntryType> {
    let label = ed.canonical_or_label().trim();
    if label.is_empty() || is_generic_concept(label) {
        return None;
    }
    match ed.kind.trim().to_ascii_lowercase().as_str() {
        "concept" | "decision" => Some(EntryType::Concept),
        "person" | "project" | "library" | "file_path" | "other" => Some(EntryType::Entity),
        _ => Some(EntryType::Entity),
    }
}

fn is_generic_concept(label: &str) -> bool {
    matches!(
        normalize_title_for_dedup(label).as_str(),
        "ai" | "人工智能" | "效率" | "技术" | "产品" | "工具" | "系统"
    )
}

fn find_summary_page(
    pages: &HashMap<PageId, WikiPage>,
    summary_title: &str,
    source_url: &str,
    scope: &Scope,
) -> Option<PageId> {
    let normalized_title = normalize_title_for_dedup(summary_title);
    pages
        .iter()
        .find(|(_, p)| {
            p.entry_type == Some(EntryType::Summary)
                && p.scope == *scope
                && if source_url.trim().is_empty() {
                    normalize_title_for_dedup(&p.title) == normalized_title
                } else {
                    p.source_url.as_deref() == Some(source_url) || p.markdown.contains(source_url)
                }
        })
        .map(|(pid, _)| *pid)
}

fn find_page_by_normalized_title(
    pages: &HashMap<PageId, WikiPage>,
    kind: &EntryType,
    normalized: &str,
    scope: &Scope,
) -> Option<PageId> {
    pages
        .iter()
        .find(|(_, p)| {
            p.entry_type.as_ref() == Some(kind)
                && p.scope == *scope
                && normalize_title_for_dedup(&p.title) == normalized
        })
        .map(|(pid, _)| *pid)
}

#[derive(Debug, Clone)]
struct SourceReference {
    summary_title: String,
    source_url: String,
}

fn append_source_reference(page: &mut WikiPage, source_ref: &SourceReference) -> bool {
    let line = source_reference_line(source_ref);
    if page.markdown.contains(&line)
        || (!source_ref.source_url.trim().is_empty()
            && page.markdown.contains(&source_ref.source_url))
    {
        return false;
    }
    if page.markdown.contains("\n## 来源引用\n") {
        page.markdown.push_str(&format!("\n{line}\n"));
    } else {
        page.markdown
            .push_str(&format!("\n## 来源引用\n\n{line}\n"));
    }
    page.updated_at = time::OffsetDateTime::now_utc();
    true
}

fn source_reference_line(source_ref: &SourceReference) -> String {
    if source_ref.source_url.trim().is_empty() {
        format!("- [[{}]]", source_ref.summary_title)
    } else {
        format!(
            "- [[{}]] — {}",
            source_ref.summary_title, source_ref.source_url
        )
    }
}

pub(crate) fn normalize_title_for_dedup(title: &str) -> String {
    let mut out = String::new();
    let mut last_space = true;
    for ch in title.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_space = false;
        } else if ch.is_ascii_punctuation() || ch.is_whitespace() {
            if !last_space {
                out.push(' ');
                last_space = true;
            }
        } else {
            out.push(ch);
            last_space = false;
        }
    }
    out.trim().to_string()
}

fn replace_section_body(md: &str, section: &str, body: &str) -> Option<String> {
    let header = format!("## {section}\n\n");
    let start = md.find(&header)? + header.len();
    let rest = &md[start..];
    let end_rel = rest.find("\n## ").unwrap_or(rest.len());
    let mut out = String::new();
    out.push_str(&md[..start]);
    out.push_str(body);
    out.push_str(&rest[end_rel..]);
    Some(out)
}

fn parse_plan_confidence(raw: &str) -> Confidence {
    match raw.trim().to_ascii_lowercase().as_str() {
        "high" => Confidence::High,
        "low" => Confidence::Low,
        _ => Confidence::Medium,
    }
}

fn find_entity_id_by_label(
    entities: &HashMap<EntityId, Entity>,
    label: &str,
    scope: &Scope,
) -> Option<EntityId> {
    entities
        .values()
        .find(|e| e.scope == *scope && e.label.eq_ignore_ascii_case(label))
        .map(|e| e.id)
}

fn mark_source_compiled(
    path: &Path,
    source_id: SourceId,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let new_content = rewrite_source_frontmatter(&content, source_id)?;
    if new_content != content {
        std::fs::write(path, new_content)?;
    }
    Ok(())
}

fn write_source_id(path: &Path, source_id: SourceId) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let new_content = rewrite_source_id_frontmatter(&content, source_id)?;
    if new_content != content {
        std::fs::write(path, new_content)?;
    }
    Ok(())
}

fn rewrite_source_frontmatter(
    content: &str,
    source_id: SourceId,
) -> Result<String, Box<dyn std::error::Error>> {
    let Some(rest) = content.strip_prefix("---\n") else {
        return Err("source markdown missing YAML frontmatter".into());
    };
    let Some(end_rel) = rest.find("\n---") else {
        return Err("source markdown frontmatter is not closed".into());
    };
    let frontmatter = &rest[..end_rel];
    let tail = &rest[end_rel + "\n---".len()..];
    let frontmatter = upsert_frontmatter_scalar(frontmatter, "compiled_to_wiki", "true");
    let frontmatter =
        upsert_frontmatter_scalar(&frontmatter, "source_id", &source_id.0.to_string());
    Ok(format!("---\n{frontmatter}---{tail}"))
}

fn rewrite_source_id_frontmatter(
    content: &str,
    source_id: SourceId,
) -> Result<String, Box<dyn std::error::Error>> {
    let Some(rest) = content.strip_prefix("---\n") else {
        return Err("source markdown missing YAML frontmatter".into());
    };
    let Some(end_rel) = rest.find("\n---") else {
        return Err("source markdown frontmatter is not closed".into());
    };
    let frontmatter = &rest[..end_rel];
    let tail = &rest[end_rel + "\n---".len()..];
    let frontmatter = upsert_frontmatter_scalar(frontmatter, "source_id", &source_id.0.to_string());
    Ok(format!("---\n{frontmatter}---{tail}"))
}

fn upsert_frontmatter_scalar(frontmatter: &str, key: &str, value: &str) -> String {
    let mut found = false;
    let mut out = String::new();
    for line in frontmatter.lines() {
        let is_key = line
            .trim_start()
            .strip_prefix(key)
            .and_then(|rest| rest.trim_start().strip_prefix(':'))
            .is_some();
        if is_key {
            out.push_str(&format!("{key}: {value}\n"));
            found = true;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !found {
        out.push_str(&format!("{key}: {value}\n"));
    }
    out
}

fn write_run_report(
    wiki_root: Option<&Path>,
    ok_count: usize,
    err_count: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(root) = wiki_root else {
        return Ok(());
    };
    let report_dir = root.join("reports");
    std::fs::create_dir_all(&report_dir)?;
    let now = time::OffsetDateTime::now_utc();
    let now_str = now.format(&Rfc3339)?;
    let path = report_dir.join(format!("production-wiki-compiler-{}.md", timestamp_slug()));
    let body = format!(
        "# Production Wiki Compiler Run\n\n- at: `{now_str}`\n- successful_sources: `{ok_count}`\n- failed_sources: `{err_count}`\n- note: current implementation uses existing `LlmIngestPlanV1`; concept/entity definition, key_points, related names, and typed compiler tags are expected future fields and are represented conservatively in page bodies.\n"
    );
    std::fs::write(path, body)?;
    Ok(())
}

fn parse_frontmatter_tags(frontmatter: &str, key: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut in_block = false;
    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if in_block {
            if let Some(item) = trimmed.strip_prefix("- ") {
                values.push(clean_yaml_scalar(item));
                continue;
            }
            if !line.starts_with(' ') && !line.starts_with('\t') {
                in_block = false;
            }
        }
        if let Some((raw_key, raw_val)) = trimmed.split_once(':') {
            if raw_key.trim() != key {
                continue;
            }
            let raw_val = raw_val.trim();
            if raw_val.is_empty() {
                in_block = true;
            } else if raw_val == "[]" {
                in_block = false;
            } else if let Some(inner) = raw_val.strip_prefix('[').and_then(|v| v.strip_suffix(']'))
            {
                values.extend(
                    inner
                        .split([',', '，'])
                        .map(clean_yaml_scalar)
                        .filter(|t| !t.is_empty()),
                );
                in_block = false;
            } else {
                values.extend(
                    raw_val
                        .split([',', '，'])
                        .map(clean_yaml_scalar)
                        .filter(|t| !t.is_empty()),
                );
                in_block = false;
            }
        }
    }
    values.into_iter().filter(|t| !t.is_empty()).collect()
}

fn clean_yaml_scalar(raw: &str) -> String {
    raw.trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

fn parse_frontmatter_kv(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, val)) = line.split_once(':') {
            let key = key.trim().to_string();
            let val = val.trim().to_string();
            let val = val
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(&val)
                .to_string();
            if !key.is_empty() {
                map.insert(key, val);
            }
        }
    }
    map
}

fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiki_core::{EntryStatus, LlmClaimDraft, LlmRelationDraft};

    fn test_scope() -> Scope {
        Scope::Shared {
            team_id: "wiki".into(),
        }
    }

    fn private_scope() -> Scope {
        Scope::Private {
            agent_id: "agent-a".into(),
        }
    }

    fn plan_with_entities(entities: Vec<LlmEntityDraft>) -> LlmIngestPlanV1 {
        LlmIngestPlanV1 {
            version: 1,
            summary: wiki_core::llm_ingest_plan::LlmSummaryDraft::default(),
            summary_title: "source".into(),
            summary_markdown: String::new(),
            one_sentence_summary: "one sentence".into(),
            key_insights: vec!["insight".into()],
            confidence: "medium".into(),
            tags: vec!["tag".into()],
            source_author: None,
            source_publisher: None,
            source_published_at: None,
            claims: vec![LlmClaimDraft {
                text: "claim".into(),
                tier: "semantic".into(),
                tags: vec![],
            }],
            concepts: Vec::new(),
            entities,
            relationships: Vec::<LlmRelationDraft>::new(),
        }
    }

    #[test]
    fn title_normalization_folds_ascii_case_punctuation_and_space() {
        assert_eq!(
            normalize_title_for_dedup("  OpenAI--Responses_API  "),
            "openai responses api"
        );
    }

    #[test]
    fn source_id_parser_accepts_uuid() {
        let raw = "00353feb-f9c4-5e2c-aa4b-1c86c8524be7".to_string();
        assert_eq!(
            parse_source_id(Some(&raw)).unwrap().0.to_string(),
            "00353feb-f9c4-5e2c-aa4b-1c86c8524be7"
        );
        assert!(parse_source_id(Some(&"not-a-uuid".to_string())).is_none());
        assert!(parse_source_id(None).is_none());
    }

    #[test]
    fn materialization_creates_summary_concept_and_entity_pages() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let plan = plan_with_entities(vec![
            LlmEntityDraft {
                label: "Agent Memory".into(),
                kind: "concept".into(),
                canonical_name: "Agent Memory".into(),
                category: None,
                definition: String::new(),
                profile: String::new(),
                key_points: Vec::new(),
                tags: Vec::new(),
                related_names: Vec::new(),
            },
            LlmEntityDraft {
                label: "Mempalace".into(),
                kind: "project".into(),
                canonical_name: "Mempalace".into(),
                category: None,
                definition: String::new(),
                profile: String::new(),
                key_points: Vec::new(),
                tags: Vec::new(),
                related_names: Vec::new(),
            },
        ]);
        let batch = BatchIngestContext {
            source_title: "Source A".into(),
            source_url: "https://example.test/a".into(),
            source_tags: vec![],
        };

        let stats = materialize_compiler_pages(
            &mut eng,
            &plan,
            &batch,
            "https://example.test/a",
            &test_scope(),
            &DomainSchema::permissive_default(),
        );

        assert!(stats.summary_created);
        assert_eq!(stats.concepts_created, 1);
        assert_eq!(stats.entities_created, 1);
        assert!(eng
            .store
            .pages
            .values()
            .any(|p| p.entry_type == Some(EntryType::Summary)
                && p.markdown.contains("[[Agent Memory]]")
                && p.markdown.contains("[[Mempalace]]")));
        assert_eq!(
            eng.outbox
                .iter()
                .filter(|event| matches!(event, wiki_core::WikiEvent::PageWritten { .. }))
                .count(),
            3
        );
    }

    #[test]
    fn existing_concept_gets_one_deduped_source_reference() {
        let source_ref = SourceReference {
            summary_title: "摘要：Source A".into(),
            source_url: "https://example.test/a".into(),
        };
        let mut page = WikiPage::new("Agent Memory", "# Agent Memory\n", test_scope())
            .with_entry_type(EntryType::Concept)
            .with_status(EntryStatus::Draft);

        assert!(append_source_reference(&mut page, &source_ref));
        assert!(!append_source_reference(&mut page, &source_ref));
        assert_eq!(page.markdown.matches("https://example.test/a").count(), 1);
    }

    #[test]
    fn duplicate_summary_source_is_skipped() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let existing = WikiPage::new(
            "摘要：Source A",
            "## 原始文章信息\n\nhttps://example.test/a\n",
            test_scope(),
        )
        .with_entry_type(EntryType::Summary)
        .with_status(EntryStatus::Approved);
        eng.store.pages.insert(existing.id, existing);
        let batch = BatchIngestContext {
            source_title: "Source A".into(),
            source_url: "https://example.test/a".into(),
            source_tags: vec![],
        };
        let stats = materialize_compiler_pages(
            &mut eng,
            &plan_with_entities(Vec::new()),
            &batch,
            "https://example.test/a",
            &test_scope(),
            &DomainSchema::permissive_default(),
        );

        assert!(!stats.summary_created);
        assert_eq!(
            eng.store
                .pages
                .values()
                .filter(|p| p.entry_type == Some(EntryType::Summary))
                .count(),
            1
        );
    }

    #[test]
    fn same_title_different_source_creates_distinct_summary() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let mut existing = WikiPage::new(
            "摘要：Source A",
            "## 原始文章信息\n\nhttps://example.test/a\n",
            test_scope(),
        )
        .with_entry_type(EntryType::Summary)
        .with_status(EntryStatus::Approved);
        existing.source_url = Some("https://example.test/a".into());
        eng.store.pages.insert(existing.id, existing);
        let batch = BatchIngestContext {
            source_title: "Source A".into(),
            source_url: "https://example.test/b".into(),
            source_tags: vec![],
        };

        let stats = materialize_compiler_pages(
            &mut eng,
            &plan_with_entities(Vec::new()),
            &batch,
            "https://example.test/b",
            &test_scope(),
            &DomainSchema::permissive_default(),
        );

        assert!(stats.summary_created);
        assert_eq!(
            eng.store
                .pages
                .values()
                .filter(|p| p.entry_type == Some(EntryType::Summary))
                .count(),
            2
        );
    }

    #[test]
    fn page_dedup_is_scoped() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let existing = WikiPage::new("Agent Memory", "# Agent Memory\n", private_scope())
            .with_entry_type(EntryType::Concept)
            .with_status(EntryStatus::Draft);
        eng.store.pages.insert(existing.id, existing);
        let plan = plan_with_entities(vec![LlmEntityDraft {
            label: "Agent Memory".into(),
            kind: "concept".into(),
            canonical_name: "Agent Memory".into(),
            category: None,
            definition: "Scoped concept".into(),
            profile: String::new(),
            key_points: Vec::new(),
            tags: Vec::new(),
            related_names: Vec::new(),
        }]);
        let batch = BatchIngestContext {
            source_title: "Source A".into(),
            source_url: "https://example.test/a".into(),
            source_tags: vec![],
        };

        let stats = materialize_compiler_pages(
            &mut eng,
            &plan,
            &batch,
            "https://example.test/a",
            &test_scope(),
            &DomainSchema::permissive_default(),
        );

        assert_eq!(stats.concepts_created, 1);
        assert_eq!(
            eng.store
                .pages
                .values()
                .filter(|p| p.entry_type == Some(EntryType::Concept)
                    && normalize_title_for_dedup(&p.title) == "agent memory")
                .count(),
            2
        );
    }

    #[test]
    fn source_compiled_rewrite_only_touches_frontmatter() {
        let source_id =
            SourceId(uuid::Uuid::parse_str("00353feb-f9c4-5e2c-aa4b-1c86c8524be7").unwrap());
        let content =
            "---\ntitle: \"A\"\ncompiled_to_wiki: false\n---\n\nbody compiled_to_wiki: false\n";

        let out = rewrite_source_frontmatter(content, source_id).unwrap();

        assert!(out.contains("compiled_to_wiki: true\n"));
        assert!(out.contains("source_id: 00353feb-f9c4-5e2c-aa4b-1c86c8524be7\n"));
        assert!(out.contains("body compiled_to_wiki: false"));
    }

    #[test]
    fn source_id_rewrite_does_not_mark_compiled() {
        let source_id =
            SourceId(uuid::Uuid::parse_str("00353feb-f9c4-5e2c-aa4b-1c86c8524be7").unwrap());
        let content = "---\ntitle: \"A\"\ncompiled_to_wiki: false\n---\n\nbody\n";

        let out = rewrite_source_id_frontmatter(content, source_id).unwrap();

        assert!(out.contains("source_id: 00353feb-f9c4-5e2c-aa4b-1c86c8524be7\n"));
        assert!(out.contains("compiled_to_wiki: false\n"));
    }

    #[test]
    fn existing_source_lookup_uses_uri_and_scope() {
        let mut sources = HashMap::new();
        let hidden = RawArtifact::new("https://example.test/a", "hidden", private_scope());
        let visible = RawArtifact::new("https://example.test/a", "visible", test_scope());
        sources.insert(hidden.id, hidden);
        sources.insert(visible.id, visible.clone());

        assert_eq!(
            find_existing_source_id_by_uri(&sources, "https://example.test/a", &test_scope()),
            Some(visible.id)
        );
    }

    #[test]
    fn frontmatter_tags_parser_supports_inline_and_block_lists() {
        assert_eq!(
            parse_frontmatter_tags("tags: [Agent, LLM, 自动化]\n", "tags"),
            vec!["Agent", "LLM", "自动化"]
        );
        assert_eq!(
            parse_frontmatter_tags("tags:\n  - \"Agent\"\n  - '长期记忆'\nurl: x\n", "tags"),
            vec!["Agent", "长期记忆"]
        );
        assert!(parse_frontmatter_tags("tags: []\n", "tags").is_empty());
    }

    #[test]
    fn relationship_entity_lookup_is_scoped() {
        let mut entities = HashMap::new();
        let hidden = Entity {
            id: EntityId(uuid::Uuid::parse_str("11111111-1111-5111-8111-111111111111").unwrap()),
            kind: EntityKind::Project,
            label: "Same".into(),
            scope: private_scope(),
        };
        let visible = Entity {
            id: EntityId(uuid::Uuid::parse_str("22222222-2222-5222-8222-222222222222").unwrap()),
            kind: EntityKind::Project,
            label: "Same".into(),
            scope: test_scope(),
        };
        entities.insert(hidden.id, hidden);
        entities.insert(visible.id, visible.clone());

        assert_eq!(
            find_entity_id_by_label(&entities, "same", &test_scope()),
            Some(visible.id)
        );
    }

    #[test]
    fn rich_summary_confidence_populates_page_metadata() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let mut plan = plan_with_entities(Vec::new());
        plan.confidence.clear();
        plan.summary.confidence = "high".into();
        let batch = BatchIngestContext {
            source_title: "Source A".into(),
            source_url: "https://example.test/a".into(),
            source_tags: vec![],
        };

        materialize_compiler_pages(
            &mut eng,
            &plan,
            &batch,
            "https://example.test/a",
            &test_scope(),
            &DomainSchema::permissive_default(),
        );

        let summary = eng
            .store
            .pages
            .values()
            .find(|p| p.entry_type == Some(EntryType::Summary))
            .unwrap();
        assert_eq!(summary.confidence, Confidence::High);
    }

    #[test]
    fn generic_concepts_are_rejected() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let batch = BatchIngestContext {
            source_title: "Source A".into(),
            source_url: "https://example.test/a".into(),
            source_tags: vec![],
        };
        let stats = materialize_compiler_pages(
            &mut eng,
            &plan_with_entities(vec![LlmEntityDraft {
                label: "AI".into(),
                kind: "concept".into(),
                canonical_name: "AI".into(),
                category: None,
                definition: String::new(),
                profile: String::new(),
                key_points: Vec::new(),
                tags: Vec::new(),
                related_names: Vec::new(),
            }]),
            &batch,
            "https://example.test/a",
            &test_scope(),
            &DomainSchema::permissive_default(),
        );

        assert_eq!(stats.concepts_created, 0);
        assert_eq!(stats.warnings, vec!["skip generic entity/concept: AI"]);
    }
}
