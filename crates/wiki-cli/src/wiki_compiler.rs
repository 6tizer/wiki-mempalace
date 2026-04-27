use crate::llm;
use crate::{parse_scope, timestamp_slug};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use wiki_core::llm_ingest_plan::LlmConceptDraft;
use wiki_core::{
    normalize_and_validate_tag_groups, parse_memory_tier, Confidence, DomainSchema, Entity,
    EntityId, EntityKind, EntryType, LlmEntityDraft, LlmIngestPlanV1, MemoryTier, PageContract,
    PageId, RawArtifact, RelationKind, Scope, SourceId, TypedEdge, WikiPage,
};
use wiki_kernel::{
    format_claim_doc_id, initial_status_for, write_projection_pages, LlmWikiEngine, NoopWikiHook,
};
use wiki_storage::{CanonicalAliasMapping, SqliteRepository};

const MIN_COMPILER_SOURCE_BODY_CHARS: usize = 300;

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

#[derive(Debug, Clone, Serialize)]
struct CompilerSourceFailure {
    source_title: String,
    source_url: Option<String>,
    source_path: String,
    error: String,
}

impl CompilerSourceFailure {
    fn new(src: &SourceEntry, error: String) -> Self {
        Self {
            source_title: src.title.clone(),
            source_url: (!src.url.is_empty()).then(|| src.url.clone()),
            source_path: src.path.display().to_string(),
            error,
        }
    }
}

#[derive(Debug, Default)]
struct PageMaterializationStats {
    summary_created: bool,
    concepts_created: usize,
    concepts_updated: usize,
    entities_created: usize,
    entities_updated: usize,
    written_page_ids: Vec<PageId>,
    alias_mappings: Vec<CanonicalAliasMapping>,
    warnings: Vec<String>,
    deferred_resolutions: Vec<DeferredResolutionItem>,
}

/// 单条 source 编译结果。
struct IngestOneStats {
    claims: usize,
    entities: usize,
    relationships: usize,
    source_id: SourceId,
    written_page_ids: Vec<PageId>,
    page_stats: PageMaterializationStats,
}

#[derive(Debug, Clone, Serialize)]
struct DeferredResolutionItem {
    source_title: Option<String>,
    source_url: Option<String>,
    draft_title: String,
    draft_entry_type: String,
    reason: String,
    candidate_count: usize,
    candidates: Vec<DeferredResolutionCandidate>,
    owner: &'static str,
    next_step: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct DeferredResolutionCandidate {
    page_id: PageId,
    title: String,
    entry_type: String,
    score: i32,
    match_reasons: Vec<String>,
}

impl DeferredResolutionItem {
    fn from_candidates(
        title: &str,
        entry_type: &EntryType,
        reason: &str,
        candidates: &[CompilerCandidate],
    ) -> Self {
        Self {
            source_title: None,
            source_url: None,
            draft_title: title.to_string(),
            draft_entry_type: entry_type_label(entry_type).to_string(),
            reason: reason.to_string(),
            candidate_count: candidates.len(),
            candidates: candidates
                .iter()
                .map(|candidate| DeferredResolutionCandidate {
                    page_id: candidate.page_id,
                    title: candidate.title.clone(),
                    entry_type: entry_type_label(&candidate.entry_type).to_string(),
                    score: candidate.score,
                    match_reasons: candidate.match_reasons.clone(),
                })
                .collect(),
            owner: "resolver-lint-fixer",
            next_step: "machine_resolution",
        }
    }

    fn with_source(&self, source: &SourceEntry) -> Self {
        let mut item = self.clone();
        item.source_title = Some(source.title.clone());
        item.source_url = if source.url.trim().is_empty() {
            None
        } else {
            Some(source.url.clone())
        };
        item
    }
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
                s.body.chars().count()
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
    let compiler_schema = trusted_compiler_schema(schema);
    eng.schema = compiler_schema.clone();
    let mut runner = WikiCompilerRunner {
        eng,
        repo,
        cfg: &cfg,
        vectors,
        llm_config_path,
        schema: &compiler_schema,
        scope,
    };

    let mut compiled_paths: Vec<(PathBuf, SourceId)> = Vec::new();
    let mut written_page_ids: HashSet<PageId> = HashSet::new();
    let mut report_warnings = Vec::new();
    let mut report_deferred_resolutions = Vec::new();
    let mut report_failures = Vec::new();
    let mut ok_count = 0usize;
    let mut err_count = 0usize;

    for (i, src) in sources.iter().enumerate() {
        heartbeat.tick();
        eprintln!("[{}/{}] {}...", i + 1, sources.len(), src.title);
        let started = std::time::Instant::now();

        match runner.compile_source(src) {
            Ok(stats) => {
                eprintln!(
                    "  ✓ claims={} entities={} rels={} source={} summary={} concepts={}/{} entities={}/{} elapsed={:.1}s",
                    stats.claims,
                    stats.entities,
                    stats.relationships,
                    stats.source_id.0,
                    if stats.page_stats.summary_created { "created" } else { "dedup" },
                    stats.page_stats.concepts_created,
                    stats.page_stats.concepts_updated,
                    stats.page_stats.entities_created,
                    stats.page_stats.entities_updated,
                    started.elapsed().as_secs_f32(),
                );
                for warning in &stats.page_stats.warnings {
                    eprintln!("  ! {warning}");
                    report_warnings.push(format!("{}: {warning}", src.title));
                }
                for deferred in &stats.page_stats.deferred_resolutions {
                    eprintln!(
                        "  ? defer resolver item: {} {} ({}; candidates={})",
                        deferred.draft_entry_type,
                        deferred.draft_title,
                        deferred.reason,
                        deferred.candidate_count
                    );
                    report_deferred_resolutions.push(deferred.with_source(src));
                }
                compiled_paths.push((src.path.clone(), stats.source_id));
                written_page_ids.extend(stats.written_page_ids);
                ok_count += 1;
            }
            Err(e) => {
                let message = e.to_string();
                eprintln!(
                    "  ✗ 失败 elapsed={:.1}s：{message}",
                    started.elapsed().as_secs_f32()
                );
                report_failures.push(CompilerSourceFailure::new(src, message));
                err_count += 1;
            }
        }

        if i + 1 < sources.len() && opts.delay_secs > 0 {
            std::thread::sleep(std::time::Duration::from_secs(opts.delay_secs));
        }
    }

    if ok_count > 0 && opts.sync_wiki {
        if let Some(root) = opts.wiki_root {
            let stats = write_projection_pages(root, &runner.eng.store, &written_page_ids)?;
            println!(
                "projection pages={} claims={} sources={}",
                stats.pages_written, stats.claims_written, stats.sources_written
            );
        }
    }
    if ok_count > 0 || err_count > 0 {
        write_run_report(
            opts.wiki_root.or(Some(opts.vault)),
            ok_count,
            err_count,
            &report_warnings,
            &report_deferred_resolutions,
            &report_failures,
        )?;
    }
    if ok_count > 0 {
        for (path, source_id) in compiled_paths {
            mark_source_compiled(&path, source_id)?;
        }
    }
    eprintln!("\n完成：成功={ok_count} 失败={err_count}");
    if err_count > 0 {
        return Err(format!(
            "batch-ingest failed for {err_count} source(s); successful_sources={ok_count}"
        )
        .into());
    }
    Ok(())
}

fn trusted_compiler_schema(schema: &DomainSchema) -> DomainSchema {
    let mut schema = schema.clone();
    schema.tag_config.max_new_tags_per_ingest = u32::MAX;
    schema.tag_config.deprecated_tags.clear();
    schema
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

        let alias_mappings =
            load_compiler_alias_mappings(self.repo, &self.eng.store.pages, &self.scope)?;
        let mut resolver = CompilerResolver::new(&self.eng.store.pages, &self.scope)
            .with_mappings(alias_mappings)
            .with_llm(self.cfg);
        let page_stats = materialize_compiler_pages_with_resolver(
            self.eng,
            &plan,
            &batch,
            &uri,
            &self.scope,
            self.schema,
            &mut resolver,
        );
        let snapshot = self.eng.store.to_snapshot(&self.eng.audits);
        self.repo.save_snapshot_and_append_outbox_with_aliases(
            &snapshot,
            &self.eng.outbox,
            &page_stats.alias_mappings,
        )?;
        self.eng.outbox.clear();

        Ok(IngestOneStats {
            claims: plan.claims.len(),
            entities: plan.entities.len(),
            relationships: plan.relationships.len(),
            source_id: sid,
            written_page_ids: page_stats.written_page_ids.clone(),
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

        let body_chars = body.chars().count();
        if body_chars < MIN_COMPILER_SOURCE_BODY_CHARS {
            eprintln!(
                "  跳过（正文过短 < {MIN_COMPILER_SOURCE_BODY_CHARS} 字符）：{}",
                title
            );
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

#[cfg(test)]
fn materialize_compiler_pages(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    plan: &LlmIngestPlanV1,
    batch: &BatchIngestContext,
    uri: &str,
    scope: &Scope,
    schema: &DomainSchema,
) -> PageMaterializationStats {
    let mut resolver = CompilerResolver::new(&eng.store.pages, scope);
    materialize_compiler_pages_with_resolver(eng, plan, batch, uri, scope, schema, &mut resolver)
}

fn materialize_compiler_pages_with_resolver(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    plan: &LlmIngestPlanV1,
    batch: &BatchIngestContext,
    uri: &str,
    scope: &Scope,
    schema: &DomainSchema,
    resolver: &mut CompilerResolver<'_>,
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

    let existing_summary = find_summary_page(
        &eng.store.pages,
        &summary_title,
        &source_ref.source_url,
        scope,
    );
    if existing_summary.is_some() {
        return stats;
    }

    let mut summary_links = Vec::new();
    let mut seen_summary_links = HashSet::new();
    for concept in &plan.concepts {
        let name = concept.canonical_name.trim();
        if name.is_empty() || is_generic_concept(name) {
            stats
                .warnings
                .push(format!("skip generic entity/concept: {name}"));
            continue;
        }
        if is_low_signal_source_local_concept(name) {
            stats
                .warnings
                .push(format!("skip low-signal concept: {name}"));
            continue;
        }
        let decision = resolver.resolve(&DraftResolverItem::concept(concept));
        match decision {
            ResolverDecision::ResolvedExisting(resolved) => {
                if resolved.persist_alias {
                    if let Some(mapping) = canonical_alias_mapping_for_resolution(
                        name,
                        &resolved,
                        scope,
                        "compiler-resolver",
                    ) {
                        stats.alias_mappings.push(mapping);
                    }
                }
                let mut updated_page = None;
                if let Some(page) = eng.store.pages.get_mut(&resolved.page_id) {
                    if append_source_reference(page, &source_ref) {
                        updated_page = Some(page.clone());
                    }
                    push_unique_link(&mut summary_links, &mut seen_summary_links, &page.title);
                }
                if let Some(page) = updated_page {
                    let page_id = page.id;
                    eng.write_page(page, "batch-ingest");
                    stats.written_page_ids.push(page_id);
                    match resolved.entry_type {
                        EntryType::Concept => stats.concepts_updated += 1,
                        EntryType::Entity => stats.entities_updated += 1,
                        _ => {}
                    }
                }
                continue;
            }
            ResolverDecision::DeferredResolution {
                title,
                entry_type,
                candidates,
                reason,
            } => {
                stats
                    .deferred_resolutions
                    .push(DeferredResolutionItem::from_candidates(
                        &title,
                        &entry_type,
                        &reason,
                        &candidates,
                    ));
                continue;
            }
            ResolverDecision::CreateNew {
                title,
                confidence,
                reason,
                ..
            } => {
                let _create_note = (&confidence, &reason);
                let page = build_concept_page(concept, &title, &source_ref, scope, schema);
                let page_id = page.id;
                if let Some(mapping) = canonical_alias_mapping_for_new_page(
                    name,
                    &page,
                    scope,
                    "compiler-resolver: create-new",
                ) {
                    stats.alias_mappings.push(mapping);
                }
                push_unique_link(&mut summary_links, &mut seen_summary_links, &page.title);
                resolver.register_page(&page);
                eng.write_page(page, "batch-ingest");
                stats.written_page_ids.push(page_id);
                stats.concepts_created += 1;
                continue;
            }
        }
    }

    for ed in &plan.entities {
        let entity_label = ed.canonical_or_label();
        if entity_declares_concept_kind(ed) && is_low_signal_source_local_concept(entity_label) {
            stats
                .warnings
                .push(format!("skip low-signal concept: {entity_label}"));
            continue;
        }
        let Some(kind) = compiler_page_kind(ed) else {
            stats
                .warnings
                .push(format!("skip generic entity/concept: {}", ed.label));
            continue;
        };
        let decision = resolver.resolve(&DraftResolverItem::entity(ed, kind.clone()));
        match decision {
            ResolverDecision::ResolvedExisting(resolved) => {
                if resolved.persist_alias {
                    if let Some(mapping) = canonical_alias_mapping_for_resolution(
                        ed.canonical_or_label(),
                        &resolved,
                        scope,
                        "compiler-resolver",
                    ) {
                        stats.alias_mappings.push(mapping);
                    }
                }
                let mut updated_page = None;
                if let Some(page) = eng.store.pages.get_mut(&resolved.page_id) {
                    if append_source_reference(page, &source_ref) {
                        updated_page = Some(page.clone());
                    }
                    push_unique_link(&mut summary_links, &mut seen_summary_links, &page.title);
                }
                if let Some(page) = updated_page {
                    let page_id = page.id;
                    eng.write_page(page, "batch-ingest");
                    stats.written_page_ids.push(page_id);
                    match resolved.entry_type {
                        EntryType::Concept => stats.concepts_updated += 1,
                        EntryType::Entity => stats.entities_updated += 1,
                        _ => {}
                    }
                }
                continue;
            }
            ResolverDecision::DeferredResolution {
                title,
                entry_type,
                candidates,
                reason,
            } => {
                stats
                    .deferred_resolutions
                    .push(DeferredResolutionItem::from_candidates(
                        &title,
                        &entry_type,
                        &reason,
                        &candidates,
                    ));
                continue;
            }
            ResolverDecision::CreateNew {
                title,
                entry_type,
                confidence,
                reason,
            } => {
                let _create_note = (&confidence, &reason);
                let page = build_concept_entity_page(
                    ed,
                    &title,
                    entry_type.clone(),
                    &source_ref,
                    scope,
                    schema,
                );
                let page_id = page.id;
                if let Some(mapping) = canonical_alias_mapping_for_new_page(
                    ed.canonical_or_label(),
                    &page,
                    scope,
                    "compiler-resolver: create-new",
                ) {
                    stats.alias_mappings.push(mapping);
                }
                push_unique_link(&mut summary_links, &mut seen_summary_links, &page.title);
                resolver.register_page(&page);
                eng.write_page(page, "batch-ingest");
                stats.written_page_ids.push(page_id);
                match entry_type {
                    EntryType::Concept => stats.concepts_created += 1,
                    EntryType::Entity => stats.entities_created += 1,
                    _ => {}
                }
            }
        }
    }

    if plan.should_materialize_summary_page() {
        let md = render_summary_markdown(plan, &source_ref.source_url, &summary_links);
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
        let page_id = page.id;
        eng.write_page(page, "batch-ingest");
        stats.written_page_ids.push(page_id);
        stats.summary_created = true;
    }
    stats
}

fn build_concept_page(
    concept: &LlmConceptDraft,
    title: &str,
    source_ref: &SourceReference,
    scope: &Scope,
    schema: &DomainSchema,
) -> WikiPage {
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
        .with_section("本文语境", render_related_names(&concept.related_names))
        .with_section("来源引用", source_reference_line(source_ref))
        .into_page(scope.clone(), status)
}

fn build_concept_entity_page(
    ed: &LlmEntityDraft,
    title: &str,
    kind: EntryType,
    source_ref: &SourceReference,
    scope: &Scope,
    schema: &DomainSchema,
) -> WikiPage {
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
        .with_section("本文语境", render_related_names(&ed.related_names))
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

fn render_related_names(names: &[String]) -> String {
    let lines: Vec<_> = names
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

fn render_summary_markdown(
    plan: &LlmIngestPlanV1,
    foot_url: &str,
    resolved_links: &[String],
) -> String {
    let mut md = plan.to_five_section_summary_body(Some(foot_url));
    let links = if resolved_links.is_empty() {
        "（暂无）".to_string()
    } else {
        resolved_links
            .iter()
            .map(|name| format!("- [[{name}]]"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    md = replace_section_body(&md, "提取的概念", &links).unwrap_or(md);
    md
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

fn entity_declares_concept_kind(ed: &LlmEntityDraft) -> bool {
    matches!(
        ed.kind.trim().to_ascii_lowercase().as_str(),
        "concept" | "decision"
    )
}

fn is_generic_concept(label: &str) -> bool {
    matches!(
        normalize_title_for_dedup(label).as_str(),
        "ai" | "人工智能" | "效率" | "技术" | "产品" | "工具" | "系统"
    )
}

fn is_low_signal_source_local_concept(label: &str) -> bool {
    matches!(
        compact_title_key(label).as_str(),
        "premium付费订阅" | "早期访问" | "话题订阅"
    )
}

#[derive(Debug, Clone)]
struct ResolvedExistingPage {
    page_id: PageId,
    title: String,
    entry_type: EntryType,
    confidence: ResolutionConfidence,
    reason: String,
    persist_alias: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ResolutionConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone)]
enum ResolverDecision {
    ResolvedExisting(ResolvedExistingPage),
    CreateNew {
        title: String,
        entry_type: EntryType,
        confidence: ResolutionConfidence,
        reason: String,
    },
    DeferredResolution {
        title: String,
        entry_type: EntryType,
        candidates: Vec<CompilerCandidate>,
        reason: String,
    },
}

#[derive(Debug, Clone)]
struct DraftResolverItem {
    title: String,
    entry_type: EntryType,
    body_hint: String,
}

impl DraftResolverItem {
    fn concept(concept: &LlmConceptDraft) -> Self {
        Self {
            title: concept.canonical_name.trim().to_string(),
            entry_type: EntryType::Concept,
            body_hint: truncate_chars(&concept.definition, 700),
        }
    }

    fn entity(entity: &LlmEntityDraft, entry_type: EntryType) -> Self {
        Self {
            title: entity.canonical_or_label().trim().to_string(),
            entry_type,
            body_hint: truncate_chars(entity.profile_or_definition(), 700),
        }
    }
}

#[derive(Debug, Clone)]
struct CompilerAliasMapping {
    normalized_alias_key: String,
    canonical_page_id: PageId,
    canonical_title: String,
    entry_type: EntryType,
    scope_key: String,
    confidence: ResolutionConfidence,
}

#[derive(Debug, Clone)]
struct CompilerCandidate {
    page_id: PageId,
    title: String,
    entry_type: EntryType,
    aliases: Vec<String>,
    excerpt: String,
    score: i32,
    match_reasons: Vec<String>,
}

struct CompilerResolver<'a> {
    candidates: Vec<CompilerCandidate>,
    mappings: Vec<CompilerAliasMapping>,
    scope_key: String,
    top_k: usize,
    llm: Option<&'a llm::LlmConfig>,
}

const COMPILER_RESOLVER_TOP_K: usize = 5;

impl<'a> CompilerResolver<'a> {
    fn new(pages: &HashMap<PageId, WikiPage>, scope: &Scope) -> Self {
        let scope_key = scope_key(scope);
        let candidates = pages
            .iter()
            .filter_map(|(pid, page)| {
                let entry_type = page.entry_type.clone()?;
                if page.scope != *scope || !is_resolver_page_type(&entry_type) {
                    return None;
                }
                Some(CompilerCandidate {
                    page_id: *pid,
                    title: page.title.clone(),
                    entry_type,
                    aliases: extract_page_alias_hints(page),
                    excerpt: resolver_candidate_excerpt(page),
                    score: 0,
                    match_reasons: Vec::new(),
                })
            })
            .collect();
        Self {
            candidates,
            mappings: Vec::new(),
            scope_key,
            top_k: COMPILER_RESOLVER_TOP_K,
            llm: None,
        }
    }

    fn with_mappings(mut self, mappings: Vec<CompilerAliasMapping>) -> Self {
        self.mappings = mappings;
        self
    }

    fn with_llm(mut self, cfg: &'a llm::LlmConfig) -> Self {
        self.llm = Some(cfg);
        self
    }

    fn register_page(&mut self, page: &WikiPage) {
        let Some(entry_type) = page.entry_type.clone() else {
            return;
        };
        if !is_resolver_page_type(&entry_type) || scope_key(&page.scope) != self.scope_key {
            return;
        }
        self.candidates.push(CompilerCandidate {
            page_id: page.id,
            title: page.title.clone(),
            entry_type,
            aliases: extract_page_alias_hints(page),
            excerpt: resolver_candidate_excerpt(page),
            score: 0,
            match_reasons: Vec::new(),
        });
    }

    fn resolve(&self, item: &DraftResolverItem) -> ResolverDecision {
        let canonical_title = canonical_output_title(&item.entry_type, &item.title);
        let requested_keys = dedup_keys_for_title(&item.title);
        let canonical_keys = dedup_keys_for_title(&canonical_title);
        let all_keys: HashSet<String> = requested_keys
            .iter()
            .chain(canonical_keys.iter())
            .cloned()
            .collect();

        if let Some(mapped) = self.find_mapping(&all_keys) {
            return ResolverDecision::ResolvedExisting(ResolvedExistingPage {
                page_id: mapped.canonical_page_id,
                title: mapped.canonical_title.clone(),
                entry_type: mapped.entry_type.clone(),
                confidence: mapped.confidence.clone(),
                reason: "alias mapping".to_string(),
                persist_alias: true,
            });
        }

        let candidates = self.retrieve_candidates(item, &all_keys);
        if let Some(resolved) = self.resolve_deterministic(item, &candidates, true) {
            return ResolverDecision::ResolvedExisting(resolved);
        }
        if let Some(resolved) = self.resolve_deterministic(item, &candidates, false) {
            return ResolverDecision::ResolvedExisting(resolved);
        }
        if candidates.is_empty() {
            return ResolverDecision::CreateNew {
                title: canonical_title,
                entry_type: item.entry_type.clone(),
                confidence: ResolutionConfidence::Medium,
                reason: "no candidates".to_string(),
            };
        }
        if let Some(resolved) = self.resolve_with_llm(item, &candidates) {
            return ResolverDecision::ResolvedExisting(resolved);
        }
        ResolverDecision::DeferredResolution {
            title: canonical_title,
            entry_type: item.entry_type.clone(),
            candidates,
            reason: "ambiguous candidates".to_string(),
        }
    }

    fn find_mapping(&self, keys: &HashSet<String>) -> Option<&CompilerAliasMapping> {
        self.mappings.iter().find(|mapping| {
            mapping.scope_key == self.scope_key
                && keys.contains(&mapping.normalized_alias_key)
                && mapping.confidence != ResolutionConfidence::Low
        })
    }

    fn retrieve_candidates(
        &self,
        item: &DraftResolverItem,
        keys: &HashSet<String>,
    ) -> Vec<CompilerCandidate> {
        let item_hint_key = compact_title_key(&item.body_hint);
        let mut scored = Vec::new();
        for candidate in &self.candidates {
            let mut c = candidate.clone();
            let mut exact = false;
            let mut has_substantive_match = false;
            let candidate_keys = candidate_keys(candidate);
            if candidate_keys.iter().any(|key| keys.contains(key)) {
                c.score += 100;
                c.match_reasons.push("exact key".to_string());
                exact = true;
                has_substantive_match = true;
            }
            if candidate.entry_type == item.entry_type {
                c.score += 15;
                c.match_reasons.push("preferred type".to_string());
            } else {
                c.score -= 15;
                c.match_reasons.push("cross type".to_string());
            }
            let text_key = compact_title_key(&format!(
                "{} {} {}",
                candidate.title,
                candidate.aliases.join(" "),
                candidate.excerpt
            ));
            if keys
                .iter()
                .any(|key| key.len() >= 5 && text_key.contains(key))
            {
                c.score += if exact { 10 } else { 45 };
                c.match_reasons.push("page text hint".to_string());
                has_substantive_match = true;
            }
            if !item_hint_key.is_empty()
                && item_hint_key.len() >= 5
                && text_key.contains(&item_hint_key)
            {
                c.score += 20;
                c.match_reasons.push("draft hint".to_string());
                has_substantive_match = true;
            }
            if has_substantive_match && c.score > 0 {
                scored.push(c);
            }
        }
        scored.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| {
                    preferred_type_rank(&b.entry_type, &item.entry_type)
                        .cmp(&preferred_type_rank(&a.entry_type, &item.entry_type))
                })
                .then_with(|| a.title.cmp(&b.title))
        });
        scored.truncate(self.top_k);
        scored
    }

    fn resolve_deterministic(
        &self,
        item: &DraftResolverItem,
        candidates: &[CompilerCandidate],
        preferred_only: bool,
    ) -> Option<ResolvedExistingPage> {
        let exact: Vec<_> = candidates
            .iter()
            .filter(|c| c.match_reasons.iter().any(|reason| reason == "exact key"))
            .filter(|c| !preferred_only || c.entry_type == item.entry_type)
            .collect();
        if exact.len() != 1 {
            return None;
        }
        let candidate = exact[0];
        Some(ResolvedExistingPage {
            page_id: candidate.page_id,
            title: candidate.title.clone(),
            entry_type: candidate.entry_type.clone(),
            confidence: ResolutionConfidence::High,
            reason: if candidate.entry_type == item.entry_type {
                "single exact candidate".to_string()
            } else {
                "single exact cross-type candidate".to_string()
            },
            persist_alias: true,
        })
    }

    fn resolve_with_llm(
        &self,
        item: &DraftResolverItem,
        candidates: &[CompilerCandidate],
    ) -> Option<ResolvedExistingPage> {
        let cfg = self.llm?;
        let user = render_llm_fallback_user_prompt(item, candidates);
        let reply = llm::complete_chat(cfg, llm_fallback_system_prompt(), &user, 512).ok()?;
        let decision = parse_llm_fallback_decision(&reply).ok()?;
        if !decision.same_page || decision.confidence == ResolutionConfidence::Low {
            return None;
        }
        let canonical_title = decision.canonical_title.trim();
        let candidate = candidates.iter().find(|c| {
            c.title == canonical_title
                || compact_title_key(&c.title) == compact_title_key(canonical_title)
        })?;
        Some(ResolvedExistingPage {
            page_id: candidate.page_id,
            title: candidate.title.clone(),
            entry_type: candidate.entry_type.clone(),
            confidence: decision.confidence,
            reason: format!("llm fallback: {}", decision.reason),
            persist_alias: false,
        })
    }
}

fn is_resolver_page_type(entry_type: &EntryType) -> bool {
    matches!(entry_type, EntryType::Concept | EntryType::Entity)
}

fn preferred_type_rank(entry_type: &EntryType, preferred: &EntryType) -> i32 {
    if entry_type == preferred {
        1
    } else {
        0
    }
}

fn candidate_keys(candidate: &CompilerCandidate) -> HashSet<String> {
    let mut keys = dedup_keys_for_title(&candidate.title);
    for alias in &candidate.aliases {
        keys.extend(dedup_keys_for_title(alias));
    }
    keys
}

fn extract_page_alias_hints(page: &WikiPage) -> Vec<String> {
    let mut aliases = Vec::new();
    aliases.extend(page.outbound_page_titles.iter().cloned());
    for line in page.markdown.lines() {
        let trimmed = line.trim();
        let is_alias_line = ["aliases:", "alias:", "别名：", "别名:"]
            .iter()
            .any(|prefix| trimmed.to_ascii_lowercase().starts_with(prefix));
        if !is_alias_line {
            continue;
        }
        let raw = trimmed
            .split_once(':')
            .map(|(_, raw)| raw)
            .or_else(|| trimmed.split_once('：').map(|(_, raw)| raw));
        if let Some(raw) = raw {
            aliases.extend(
                raw.split([',', '，', '/', '|'])
                    .map(clean_yaml_scalar)
                    .filter(|s| !s.is_empty()),
            );
        }
    }
    aliases.sort();
    aliases.dedup();
    aliases
}

fn resolver_candidate_excerpt(page: &WikiPage) -> String {
    let md = page
        .markdown
        .split("\n## 来源引用\n")
        .next()
        .unwrap_or(&page.markdown);
    truncate_chars(md, 360)
}

fn scope_key(scope: &Scope) -> String {
    match scope {
        Scope::Private { agent_id } => format!("private:{agent_id}"),
        Scope::Shared { team_id } => format!("shared:{team_id}"),
    }
}

fn entry_type_label(entry_type: &EntryType) -> &'static str {
    match entry_type {
        EntryType::Concept => "concept",
        EntryType::Entity => "entity",
        EntryType::Summary => "summary",
        EntryType::Synthesis => "synthesis",
        EntryType::Qa => "qa",
        EntryType::LintReport => "lint_report",
        EntryType::Index => "index",
    }
}

fn llm_fallback_system_prompt() -> &'static str {
    r#"You decide if one draft wiki item and one candidate wiki page are the same canonical page.
Return only JSON:
{"same_page":true,"canonical_title":"...","confidence":"high|medium|low","reason":"..."}
Use only the provided draft item and bounded candidates. Do not assume full wiki context."#
}

fn render_llm_fallback_user_prompt(
    item: &DraftResolverItem,
    candidates: &[CompilerCandidate],
) -> String {
    let candidates_json: Vec<_> = candidates
        .iter()
        .map(|candidate| {
            serde_json::json!({
                "title": candidate.title,
                "type": entry_type_label(&candidate.entry_type),
                "aliases": candidate.aliases,
                "excerpt": candidate.excerpt,
                "score": candidate.score,
                "match_reasons": candidate.match_reasons,
            })
        })
        .collect();
    serde_json::json!({
        "draft": {
            "title": item.title,
            "type": entry_type_label(&item.entry_type),
            "definition_or_profile": item.body_hint,
        },
        "candidates": candidates_json,
    })
    .to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LlmFallbackDecision {
    same_page: bool,
    canonical_title: String,
    confidence: ResolutionConfidence,
    reason: String,
}

#[derive(Debug, Deserialize)]
struct RawLlmFallbackDecision {
    same_page: bool,
    canonical_title: String,
    confidence: String,
    reason: String,
}

fn parse_llm_fallback_decision(
    raw: &str,
) -> Result<LlmFallbackDecision, Box<dyn std::error::Error>> {
    let parsed: RawLlmFallbackDecision = serde_json::from_str(llm::parse_json_object_slice(raw))?;
    Ok(LlmFallbackDecision {
        same_page: parsed.same_page,
        canonical_title: parsed.canonical_title,
        confidence: parse_resolution_confidence(&parsed.confidence),
        reason: parsed.reason,
    })
}

fn parse_resolution_confidence(raw: &str) -> ResolutionConfidence {
    match raw.trim().to_ascii_lowercase().as_str() {
        "high" => ResolutionConfidence::High,
        "low" => ResolutionConfidence::Low,
        _ => ResolutionConfidence::Medium,
    }
}

fn load_compiler_alias_mappings(
    repo: &SqliteRepository,
    pages: &HashMap<PageId, WikiPage>,
    scope: &Scope,
) -> Result<Vec<CompilerAliasMapping>, Box<dyn std::error::Error>> {
    let page_ids: Vec<_> = pages
        .values()
        .filter(|page| page.scope == *scope)
        .filter(|page| page.entry_type.as_ref().is_some_and(is_resolver_page_type))
        .map(|page| page.id)
        .collect();
    let page_ids: HashSet<_> = page_ids.into_iter().collect();
    let mappings = repo
        .list_canonical_aliases_for_scope(scope)?
        .into_iter()
        .filter(|mapping| page_ids.contains(&mapping.canonical_page_id))
        .filter(|mapping| is_resolver_page_type(&mapping.entry_type))
        .map(compiler_alias_from_storage)
        .collect();
    Ok(mappings)
}

fn compiler_alias_from_storage(mapping: CanonicalAliasMapping) -> CompilerAliasMapping {
    CompilerAliasMapping {
        normalized_alias_key: mapping.normalized_alias_key,
        canonical_page_id: mapping.canonical_page_id,
        canonical_title: mapping.canonical_title,
        entry_type: mapping.entry_type,
        scope_key: scope_key(&mapping.scope),
        confidence: confidence_from_score(mapping.confidence),
    }
}

fn confidence_from_score(score: f64) -> ResolutionConfidence {
    if score >= 0.85 {
        ResolutionConfidence::High
    } else if score >= 0.5 {
        ResolutionConfidence::Medium
    } else {
        ResolutionConfidence::Low
    }
}

fn confidence_score(confidence: &ResolutionConfidence) -> f64 {
    match confidence {
        ResolutionConfidence::High => 0.95,
        ResolutionConfidence::Medium => 0.75,
        ResolutionConfidence::Low => 0.25,
    }
}

fn canonical_alias_mapping_for_resolution(
    alias_text: &str,
    resolved: &ResolvedExistingPage,
    scope: &Scope,
    source: &str,
) -> Option<CanonicalAliasMapping> {
    let alias_text = alias_text.trim();
    let normalized_alias_key = compact_title_key(alias_text);
    if alias_text.is_empty()
        || normalized_alias_key.is_empty()
        || normalized_alias_key == compact_title_key(&resolved.title)
    {
        return None;
    }
    let now = OffsetDateTime::now_utc();
    Some(CanonicalAliasMapping {
        alias_text: alias_text.to_string(),
        normalized_alias_key,
        canonical_page_id: resolved.page_id,
        canonical_title: resolved.title.clone(),
        entry_type: resolved.entry_type.clone(),
        scope: scope.clone(),
        source: format!("{source}: {}", resolved.reason),
        confidence: confidence_score(&resolved.confidence),
        created_at: now,
        updated_at: now,
    })
}

fn canonical_alias_mapping_for_new_page(
    alias_text: &str,
    page: &WikiPage,
    scope: &Scope,
    source: &str,
) -> Option<CanonicalAliasMapping> {
    let resolved = ResolvedExistingPage {
        page_id: page.id,
        title: page.title.clone(),
        entry_type: page.entry_type.clone()?,
        confidence: ResolutionConfidence::High,
        reason: "new canonical page".to_string(),
        persist_alias: true,
    };
    canonical_alias_mapping_for_resolution(alias_text, &resolved, scope, source)
}

fn push_unique_link(out: &mut Vec<String>, seen: &mut HashSet<String>, title: &str) {
    let key = compact_title_key(title);
    if !key.is_empty() && seen.insert(key) {
        out.push(title.to_string());
    }
}

fn canonical_output_title(kind: &EntryType, requested_title: &str) -> String {
    let title = requested_title.trim();
    if *kind == EntryType::Entity {
        if let Some(repo) = github_repo_name(title) {
            return canonical_repo_title(repo);
        }
        if let Some(stripped) = strip_entity_descriptor_suffix(title) {
            return format_cjk_ascii_boundaries(&stripped);
        }
    }
    format_cjk_ascii_boundaries(title)
}

fn github_repo_name(title: &str) -> Option<&str> {
    let trimmed = title.trim();
    let (owner, repo) = trimmed.split_once('/')?;
    if owner.trim().is_empty() || repo.trim().is_empty() || repo.contains('/') {
        return None;
    }
    Some(repo.trim())
}

fn canonical_repo_title(repo: &str) -> String {
    match compact_title_key(repo).as_str() {
        "openwebui" => "Open WebUI".to_string(),
        "anythingllm" => "AnythingLLM".to_string(),
        "crewai" => "CrewAI".to_string(),
        "n8n" => "n8n".to_string(),
        _ => repo.trim().to_string(),
    }
}

fn strip_entity_descriptor_suffix(title: &str) -> Option<String> {
    let title = title.trim();
    let mut split_at = None;
    for (idx, ch) in title.char_indices() {
        if idx == 0 {
            if !ch.is_ascii_alphanumeric() {
                return None;
            }
            continue;
        }
        if !(ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.')) {
            split_at = Some(idx);
            break;
        }
    }
    let split_at = split_at?;
    let prefix = title[..split_at].trim();
    let suffix = title[split_at..].trim();
    if prefix.is_empty()
        || suffix.is_empty()
        || matches!(compact_title_key(prefix).as_str(), "ai" | "api" | "llm")
    {
        return None;
    }
    if entity_descriptor_suffix_is_generic(suffix) {
        Some(prefix.to_string())
    } else {
        None
    }
}

fn entity_descriptor_suffix_is_generic(suffix: &str) -> bool {
    if !suffix.chars().any(is_cjk_alnum) {
        return false;
    }
    let key = compact_title_key(suffix);
    matches!(
        key.as_str(),
        "统一aiapi聚合平台"
            | "aiapi聚合平台"
            | "api聚合平台"
            | "聚合平台"
            | "平台原twitter"
            | "平台"
            | "应用"
            | "公众号"
            | "模型"
            | "项目"
            | "工具"
            | "服务"
    )
}

fn format_cjk_ascii_boundaries(title: &str) -> String {
    let mut out = String::new();
    let mut prev_kind = CharTitleKind::Other;
    for ch in title.trim().chars() {
        let kind = if ch.is_ascii_alphanumeric() {
            CharTitleKind::AsciiAlnum
        } else if is_cjk_alnum(ch) {
            CharTitleKind::CjkAlnum
        } else if ch.is_whitespace() {
            CharTitleKind::Space
        } else {
            CharTitleKind::Other
        };
        if matches!(
            (prev_kind, kind),
            (CharTitleKind::AsciiAlnum, CharTitleKind::CjkAlnum)
                | (CharTitleKind::CjkAlnum, CharTitleKind::AsciiAlnum)
        ) && !out.ends_with(' ')
        {
            out.push(' ');
        }
        if kind == CharTitleKind::Space {
            if !out.ends_with(' ') {
                out.push(' ');
            }
        } else {
            out.push(ch);
        }
        prev_kind = kind;
    }
    out.trim().to_string()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CharTitleKind {
    AsciiAlnum,
    CjkAlnum,
    Space,
    Other,
}

fn dedup_keys_for_title(title: &str) -> HashSet<String> {
    let mut keys = HashSet::new();
    let normalized = normalize_title_for_dedup(title);
    if !normalized.is_empty() {
        keys.insert(normalized);
    }
    let compact = compact_title_key(title);
    if !compact.is_empty() {
        keys.insert(compact);
    }
    if let Some(repo) = github_repo_name(title) {
        keys.extend(dedup_keys_for_title(repo));
    }
    keys
}

fn find_summary_page(
    pages: &HashMap<PageId, WikiPage>,
    summary_title: &str,
    source_url: &str,
    scope: &Scope,
) -> Option<PageId> {
    let normalized_title = normalize_title_for_dedup(summary_title);
    let source_url = source_url.trim();
    pages
        .iter()
        .find(|(_, p)| {
            let source_matches = !source_url.is_empty()
                && (p.source_url.as_deref() == Some(source_url) || p.markdown.contains(source_url));
            let has_stored_source_url = p
                .source_url
                .as_deref()
                .is_some_and(|stored| !stored.trim().is_empty());
            let title_matches = normalize_title_for_dedup(&p.title) == normalized_title
                && (source_url.is_empty() || !has_stored_source_url);
            p.entry_type == Some(EntryType::Summary)
                && p.scope == *scope
                && (title_matches || source_matches)
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
        } else if ch.is_ascii_punctuation() || ch.is_whitespace() || is_cjk_punctuation(ch) {
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

fn compact_title_key(title: &str) -> String {
    title
        .chars()
        .filter_map(|ch| {
            if ch.is_ascii_alphanumeric() {
                Some(ch.to_ascii_lowercase())
            } else if ch.is_alphanumeric() {
                Some(ch)
            } else if ch.is_whitespace() || ch.is_ascii_punctuation() || is_cjk_punctuation(ch) {
                None
            } else {
                Some(ch)
            }
        })
        .collect()
}

fn is_cjk_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '，' | '。'
            | '、'
            | '：'
            | '；'
            | '？'
            | '！'
            | '（'
            | '）'
            | '【'
            | '】'
            | '《'
            | '》'
            | '“'
            | '”'
            | '‘'
            | '’'
            | '「'
            | '」'
            | '『'
            | '』'
            | '·'
            | '—'
    )
}

fn is_cjk_alnum(ch: char) -> bool {
    ch.is_alphanumeric() && !ch.is_ascii()
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
    warnings: &[String],
    deferred_resolutions: &[DeferredResolutionItem],
    failures: &[CompilerSourceFailure],
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(root) = wiki_root else {
        return Ok(());
    };
    let report_dir = root.join("reports");
    std::fs::create_dir_all(&report_dir)?;
    let now = time::OffsetDateTime::now_utc();
    let now_str = now.format(&Rfc3339)?;
    let slug = timestamp_slug();
    let path = report_dir.join(format!("production-wiki-compiler-{slug}.md"));
    let json_path = report_dir.join(format!("production-wiki-compiler-{slug}.json"));
    let warning_block = if warnings.is_empty() {
        "- warnings: `0`\n".to_string()
    } else {
        let lines = warnings
            .iter()
            .map(|warning| format!("  - {warning}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("- warnings: `{}`\n{lines}\n", warnings.len())
    };
    let deferred_block = if deferred_resolutions.is_empty() {
        "- deferred_resolutions: `0`\n".to_string()
    } else {
        let lines = deferred_resolutions
            .iter()
            .map(|item| {
                format!(
                    "  - {} {} -> {} ({})",
                    item.draft_entry_type, item.draft_title, item.next_step, item.reason
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "- deferred_resolutions: `{}`\n{lines}\n",
            deferred_resolutions.len()
        )
    };
    let failures_block = if failures.is_empty() {
        "- source_failures: `0`\n".to_string()
    } else {
        let lines = failures
            .iter()
            .map(|failure| {
                format!(
                    "  - {} ({}) - {}",
                    failure.source_title,
                    failure.source_path,
                    truncate_chars(&failure.error, 240)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("- source_failures: `{}`\n{lines}\n", failures.len())
    };
    let body = format!(
        "# Production Wiki Compiler Run\n\n- at: `{now_str}`\n- successful_sources: `{ok_count}`\n- failed_sources: `{err_count}`\n- note: current implementation uses existing `LlmIngestPlanV1`; concept/entity definition, key_points, related names, and typed compiler tags are expected future fields and are represented conservatively in page bodies.\n"
    );
    let body = format!("{body}{warning_block}{deferred_block}{failures_block}");
    std::fs::write(path, body)?;
    let json = CompilerRunReportJson {
        version: 1,
        kind: "production_wiki_compiler_run",
        at: now_str,
        successful_sources: ok_count,
        failed_sources: err_count,
        warnings,
        deferred_resolutions,
        source_failures: failures,
    };
    std::fs::write(json_path, serde_json::to_string_pretty(&json)?)?;
    Ok(())
}

#[derive(Serialize)]
struct CompilerRunReportJson<'a> {
    version: u32,
    kind: &'static str,
    at: String,
    successful_sources: usize,
    failed_sources: usize,
    warnings: &'a [String],
    deferred_resolutions: &'a [DeferredResolutionItem],
    source_failures: &'a [CompilerSourceFailure],
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

    fn concept(name: &str) -> LlmConceptDraft {
        LlmConceptDraft {
            canonical_name: name.into(),
            kind: "concept".into(),
            definition: format!("{name} definition"),
            key_points: vec![format!("{name} point")],
            tags: Vec::new(),
            related_names: Vec::new(),
            category: None,
        }
    }

    fn entity(name: &str) -> LlmEntityDraft {
        LlmEntityDraft {
            label: name.into(),
            kind: "project".into(),
            canonical_name: name.into(),
            category: None,
            definition: format!("{name} definition"),
            profile: String::new(),
            key_points: vec![format!("{name} point")],
            tags: Vec::new(),
            related_names: Vec::new(),
        }
    }

    fn alias_mapping(alias: &str, page: &WikiPage) -> CompilerAliasMapping {
        CompilerAliasMapping {
            normalized_alias_key: compact_title_key(alias),
            canonical_page_id: page.id,
            canonical_title: page.title.clone(),
            entry_type: page.entry_type.clone().unwrap(),
            scope_key: scope_key(&page.scope),
            confidence: ResolutionConfidence::High,
        }
    }

    fn long_source_body(topic: &str) -> String {
        format!(
            "{topic} explains compiler canonicalization, resolver candidate bounds, alias mapping, lint fixer order, and DB-first projection behavior. "
        )
        .repeat(5)
    }

    #[test]
    fn compiler_alias_mappings_load_from_sqlite_for_scope_pages() {
        let dir = tempfile::tempdir().unwrap();
        let repo = SqliteRepository::open(dir.path().join("wiki.db")).unwrap();
        let page = WikiPage::new("MCP 协议", "# MCP 协议\n", test_scope())
            .with_entry_type(EntryType::Concept)
            .with_status(EntryStatus::Draft);
        let other_scope_page = WikiPage::new("MCP private", "# MCP private\n", private_scope())
            .with_entry_type(EntryType::Concept)
            .with_status(EntryStatus::Draft);
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        repo.upsert_canonical_alias(&CanonicalAliasMapping {
            alias_text: "MCP connectors".into(),
            normalized_alias_key: compact_title_key("MCP connectors"),
            canonical_page_id: page.id,
            canonical_title: page.title.clone(),
            entry_type: EntryType::Concept,
            scope: test_scope(),
            source: "unit-test".into(),
            confidence: 0.95,
            created_at: now,
            updated_at: now,
        })
        .unwrap();
        repo.upsert_canonical_alias(&CanonicalAliasMapping {
            alias_text: "MCP private".into(),
            normalized_alias_key: compact_title_key("MCP private"),
            canonical_page_id: other_scope_page.id,
            canonical_title: other_scope_page.title.clone(),
            entry_type: EntryType::Concept,
            scope: private_scope(),
            source: "unit-test".into(),
            confidence: 0.95,
            created_at: now,
            updated_at: now,
        })
        .unwrap();
        let pages = HashMap::from([(page.id, page), (other_scope_page.id, other_scope_page)]);

        let loaded = load_compiler_alias_mappings(&repo, &pages, &test_scope()).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].canonical_title, "MCP 协议");
        assert_eq!(
            loaded[0].normalized_alias_key,
            compact_title_key("MCP connectors")
        );
    }

    #[test]
    fn temp_vault_smoke_scans_x_and_wechat_samples_only() {
        let dir = tempfile::tempdir().unwrap();
        let x_dir = dir.path().join("sources/x");
        let wechat_dir = dir.path().join("sources/wechat");
        std::fs::create_dir_all(&x_dir).unwrap();
        std::fs::create_dir_all(&wechat_dir).unwrap();
        std::fs::write(
            x_dir.join("x-sample.md"),
            format!(
                "---\ntitle: \"X MCP sample\"\nurl: https://x.com/example/status/1\ncompiled_to_wiki: false\ntags: [x, MCP]\n---\n\n{}\n",
                long_source_body("This X sample")
            ),
        )
        .unwrap();
        std::fs::write(
            wechat_dir.join("wechat-sample.md"),
            format!(
                "---\ntitle: \"WeChat MCP sample\"\nurl: https://mp.weixin.qq.com/s/example\ncompiled_to_wiki: false\ntags:\n  - wechat\n  - MCP\n---\n\n{}\n",
                long_source_body("This WeChat sample")
            ),
        )
        .unwrap();

        let all = scan_uncompiled_sources(dir.path(), Some("all"), None).unwrap();
        let x = scan_uncompiled_sources(dir.path(), Some("x"), None).unwrap();
        let wechat = scan_uncompiled_sources(dir.path(), Some("wechat"), None).unwrap();

        assert_eq!(all.len(), 2);
        assert_eq!(x.len(), 1);
        assert_eq!(wechat.len(), 1);
        assert_eq!(x[0].origin.as_deref(), Some("x"));
        assert_eq!(wechat[0].origin.as_deref(), Some("wechat"));
    }

    #[test]
    fn temp_vault_smoke_skips_short_sources_before_llm() {
        let dir = tempfile::tempdir().unwrap();
        let x_dir = dir.path().join("sources/x");
        std::fs::create_dir_all(&x_dir).unwrap();
        std::fs::write(
            x_dir.join("short.md"),
            "---\ntitle: \"Short source\"\ncompiled_to_wiki: false\n---\n\nToo short.\n",
        )
        .unwrap();
        std::fs::write(
            x_dir.join("long.md"),
            format!(
                "---\ntitle: \"Long source\"\ncompiled_to_wiki: false\n---\n\n{}\n",
                long_source_body("Long source")
            ),
        )
        .unwrap();

        let entries = scan_uncompiled_sources(dir.path(), Some("all"), None).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Long source");
        assert!(entries[0].body.chars().count() >= MIN_COMPILER_SOURCE_BODY_CHARS);
    }

    #[test]
    fn run_report_persists_machine_deferred_resolutions() {
        let dir = tempfile::tempdir().unwrap();
        let deferred = DeferredResolutionItem {
            source_title: Some("Source A".to_string()),
            source_url: Some("https://example.test/a".to_string()),
            draft_title: "MCP connectors".to_string(),
            draft_entry_type: "concept".to_string(),
            reason: "ambiguous candidates".to_string(),
            candidate_count: 1,
            candidates: vec![DeferredResolutionCandidate {
                page_id: PageId(
                    uuid::Uuid::parse_str("11111111-1111-5111-8111-111111111111").unwrap(),
                ),
                title: "MCP 协议".to_string(),
                entry_type: "concept".to_string(),
                score: 100,
                match_reasons: vec!["exact key".to_string()],
            }],
            owner: "resolver-lint-fixer",
            next_step: "machine_resolution",
        };

        write_run_report(Some(dir.path()), 1, 0, &[], &[deferred], &[]).unwrap();

        let report_dir = dir.path().join("reports");
        let md_path = std::fs::read_dir(&report_dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.extension().and_then(|s| s.to_str()) == Some("md"))
            .unwrap();
        let report = std::fs::read_to_string(md_path).unwrap();
        assert!(report.contains("- warnings: `0`"));
        assert!(report.contains("- deferred_resolutions: `1`"));
        assert!(report.contains("concept MCP connectors -> machine_resolution"));

        let json_path = std::fs::read_dir(&report_dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.extension().and_then(|s| s.to_str()) == Some("json"))
            .unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(json_path).unwrap()).unwrap();
        assert_eq!(
            json["deferred_resolutions"][0]["owner"],
            "resolver-lint-fixer"
        );
        assert_eq!(
            json["deferred_resolutions"][0]["next_step"],
            "machine_resolution"
        );
    }

    #[test]
    fn run_report_persists_machine_source_failures() {
        let dir = tempfile::tempdir().unwrap();
        let failure = CompilerSourceFailure {
            source_title: "Source Failure".to_string(),
            source_url: Some("https://example.test/fail".to_string()),
            source_path: "/tmp/source-failure.md".to_string(),
            error: "JSON parse error: expected value".to_string(),
        };

        write_run_report(Some(dir.path()), 0, 1, &[], &[], &[failure]).unwrap();

        let report_dir = dir.path().join("reports");
        let md_path = std::fs::read_dir(&report_dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.extension().and_then(|s| s.to_str()) == Some("md"))
            .unwrap();
        let report = std::fs::read_to_string(md_path).unwrap();
        assert!(report.contains("- failed_sources: `1`"));
        assert!(report.contains("- source_failures: `1`"));
        assert!(report.contains("Source Failure"));

        let json_path = std::fs::read_dir(&report_dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.extension().and_then(|s| s.to_str()) == Some("json"))
            .unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(json_path).unwrap()).unwrap();
        assert_eq!(json["source_failures"][0]["source_title"], "Source Failure");
        assert_eq!(
            json["source_failures"][0]["source_url"],
            "https://example.test/fail"
        );
    }

    #[test]
    fn batch_ingest_returns_error_when_any_source_fails() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("sources/x");
        std::fs::create_dir_all(&source_dir).unwrap();
        let source_path = source_dir.join("failing.md");
        std::fs::write(
            &source_path,
            format!(
                "---\ntitle: \"Failing source\"\nurl: https://example.test/failing\ncompiled_to_wiki: false\n---\n\n{}\n",
                long_source_body("Failing source")
            ),
        )
        .unwrap();
        let llm_config_path = dir.path().join("llm-config.toml");
        std::fs::write(
            &llm_config_path,
            "[llm]\nbase_url = \"http://127.0.0.1:9\"\napi_key = \"test\"\nmodel = \"test\"\ntimeout_seconds = 1\nmax_retries = 0\n",
        )
        .unwrap();
        let repo = SqliteRepository::open(dir.path().join("wiki.db")).unwrap();
        let heartbeat = crate::AutomationHeartbeat {
            repo: &repo,
            run_id: None,
        };
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());

        let err = batch_ingest_cmd(
            &mut eng,
            &repo,
            &llm_config_path,
            false,
            &DomainSchema::permissive_default(),
            &heartbeat,
            BatchIngestOptions {
                vault: dir.path(),
                limit: None,
                dry_run: false,
                delay_secs: 0,
                sync_wiki: false,
                wiki_root: Some(dir.path()),
                scope: Some("shared:wiki"),
                origin: Some("all"),
                source_path: None,
            },
        )
        .unwrap_err();

        assert!(err.to_string().contains("batch-ingest failed for 1 source"));
        let source = std::fs::read_to_string(&source_path).unwrap();
        assert!(source.contains("compiled_to_wiki: false"));
        let json_path = std::fs::read_dir(dir.path().join("reports"))
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.extension().and_then(|s| s.to_str()) == Some("json"))
            .unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(json_path).unwrap()).unwrap();
        assert_eq!(json["successful_sources"], 0);
        assert_eq!(json["failed_sources"], 1);
        assert_eq!(json["source_failures"][0]["source_title"], "Failing source");
    }

    #[test]
    fn title_normalization_folds_ascii_case_punctuation_and_space() {
        assert_eq!(
            normalize_title_for_dedup("  OpenAI--Responses_API  "),
            "openai responses api"
        );
        assert_eq!(
            normalize_title_for_dedup("摘要：Nexu（一键）"),
            "摘要 nexu 一键"
        );
    }

    #[test]
    fn compiler_quality_rules_format_titles_and_strip_entity_descriptors() {
        assert_eq!(
            canonical_output_title(&EntryType::Concept, "统一AI API聚合平台"),
            "统一 AI API 聚合平台"
        );
        assert_eq!(
            canonical_output_title(&EntryType::Concept, "Agent群组"),
            "Agent 群组"
        );
        assert_eq!(
            canonical_output_title(&EntryType::Entity, "APIMart统一AI API聚合平台"),
            "APIMart"
        );
        assert_eq!(
            canonical_output_title(&EntryType::Entity, "X平台（原Twitter）"),
            "X"
        );
        assert_eq!(
            canonical_output_title(&EntryType::Entity, "CodeX Agent"),
            "CodeX Agent"
        );
    }

    #[test]
    fn materialization_skips_low_signal_feature_concepts() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let mut feature_entity = entity("话题订阅");
        feature_entity.kind = "concept".into();
        let mut plan =
            plan_with_entities(vec![entity("APIMart统一AI API聚合平台"), feature_entity]);
        plan.concepts = vec![concept("统一AI API聚合平台"), concept("Premium付费订阅")];
        let batch = BatchIngestContext {
            source_title: "APIMart Source".into(),
            source_url: "https://example.test/apimart".into(),
            source_tags: vec![],
        };

        let stats = materialize_compiler_pages(
            &mut eng,
            &plan,
            &batch,
            "https://example.test/apimart",
            &test_scope(),
            &DomainSchema::permissive_default(),
        );

        assert_eq!(stats.concepts_created, 1);
        assert_eq!(stats.entities_created, 1);
        assert!(stats
            .warnings
            .contains(&"skip low-signal concept: Premium付费订阅".to_string()));
        assert!(eng
            .store
            .pages
            .values()
            .any(|p| p.title == "统一 AI API 聚合平台"));
        assert!(eng.store.pages.values().any(|p| p.title == "APIMart"));
        assert!(!eng
            .store
            .pages
            .values()
            .any(|p| p.title == "Premium付费订阅"));
        assert!(!eng.store.pages.values().any(|p| p.title == "话题订阅"));
        let summary = eng
            .store
            .pages
            .values()
            .find(|p| p.title == "摘要：APIMart Source")
            .unwrap();
        assert!(summary.markdown.contains("[[统一 AI API 聚合平台]]"));
        assert!(summary.markdown.contains("[[APIMart]]"));
        assert!(!summary.markdown.contains("[[Premium付费订阅]]"));
    }

    #[test]
    fn summary_lookup_matches_normalized_title_when_source_url_was_missing() {
        let mut page = WikiPage::new(
            "摘要：nexu：一键把 OpenClaw AI Agent 接入微信和飞书的开源桌面客户端",
            "# old summary\n",
            test_scope(),
        )
        .with_entry_type(EntryType::Summary)
        .with_status(EntryStatus::Draft);
        page.source_url = None;
        let page_id = page.id;
        let pages = HashMap::from([(page_id, page)]);

        let found = find_summary_page(
            &pages,
            "摘要：Nexu：一键把 OpenClaw AI Agent 接入微信和飞书的开源桌面客户端",
            "https://x.com/nexudotio/status/2036810399341740335",
            &test_scope(),
        );

        assert_eq!(found, Some(page_id));
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
    fn related_names_are_plain_text_until_resolved() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let mut plan = plan_with_entities(Vec::new());
        plan.concepts = vec![LlmConceptDraft {
            canonical_name: "Token计费模式".into(),
            kind: "concept".into(),
            definition: "按 token 数量计费的方式".into(),
            key_points: vec!["缓存命中可降低成本".into()],
            tags: Vec::new(),
            related_names: vec!["输入token".into(), "缓存命中".into()],
            category: None,
        }];
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
        let page = eng
            .store
            .pages
            .values()
            .find(|p| p.title == "Token 计费模式")
            .unwrap();
        assert!(page.markdown.contains("- 输入token"));
        assert!(page.markdown.contains("- 缓存命中"));
        assert!(!page.markdown.contains("[[输入token]]"));
        assert!(!page.markdown.contains("[[缓存命中]]"));
    }

    #[test]
    fn in_run_created_page_is_reused_for_duplicate_draft_item() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let mut plan = plan_with_entities(Vec::new());
        plan.concepts = vec![concept("Local Context"), concept("Local Context")];
        let batch = BatchIngestContext {
            source_title: "Source Duplicate Draft".into(),
            source_url: "https://example.test/duplicate-draft".into(),
            source_tags: vec![],
        };

        let stats = materialize_compiler_pages(
            &mut eng,
            &plan,
            &batch,
            "https://example.test/duplicate-draft",
            &test_scope(),
            &DomainSchema::permissive_default(),
        );

        assert_eq!(stats.concepts_created, 1);
        assert_eq!(stats.concepts_updated, 0);
        assert_eq!(
            eng.store
                .pages
                .values()
                .filter(|p| p.entry_type == Some(EntryType::Concept) && p.title == "Local Context")
                .count(),
            1
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
    fn compiler_dedup_resolves_notion_style_aliases_before_creating_pages() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let existing_pages = vec![
            WikiPage::new("MCP-协议", "# MCP-协议\n", test_scope())
                .with_entry_type(EntryType::Concept)
                .with_status(EntryStatus::Draft),
            WikiPage::new("RAG", "# RAG\n", test_scope())
                .with_entry_type(EntryType::Concept)
                .with_status(EntryStatus::Draft),
            WikiPage::new("Ollama", "# Ollama\n", test_scope())
                .with_entry_type(EntryType::Entity)
                .with_status(EntryStatus::Draft),
            WikiPage::new("Dify", "# Dify\n", test_scope())
                .with_entry_type(EntryType::Entity)
                .with_status(EntryStatus::Draft),
            WikiPage::new("Vane", "# Vane\n\nVane 前身为 Perplexica。\n", test_scope())
                .with_entry_type(EntryType::Entity)
                .with_status(EntryStatus::Draft),
        ];
        let mappings = vec![
            alias_mapping("MCP协议", &existing_pages[0]),
            alias_mapping("MCP（Model Context Protocol）", &existing_pages[0]),
            alias_mapping("检索增强生成", &existing_pages[1]),
            alias_mapping("ollama/ollama", &existing_pages[2]),
            alias_mapping("langgenius/dify", &existing_pages[3]),
            alias_mapping("ItzCrazyKns/Perplexica", &existing_pages[4]),
        ];
        for page in &existing_pages {
            eng.store.pages.insert(page.id, page.clone());
        }
        let mut plan = plan_with_entities(vec![
            entity("ollama/ollama"),
            entity("langgenius/dify"),
            entity("ItzCrazyKns/Perplexica"),
        ]);
        plan.concepts = vec![
            concept("MCP协议"),
            concept("MCP（Model Context Protocol）"),
            concept("检索增强生成"),
        ];
        let batch = BatchIngestContext {
            source_title: "Source Alias".into(),
            source_url: "https://example.test/alias".into(),
            source_tags: vec![],
        };

        let mut resolver =
            CompilerResolver::new(&eng.store.pages, &test_scope()).with_mappings(mappings);
        let stats = materialize_compiler_pages_with_resolver(
            &mut eng,
            &plan,
            &batch,
            "https://example.test/alias",
            &test_scope(),
            &DomainSchema::permissive_default(),
            &mut resolver,
        );

        assert!(stats.summary_created);
        assert_eq!(stats.concepts_created, 0);
        assert_eq!(stats.entities_created, 0);
        assert_eq!(stats.concepts_updated, 2);
        assert_eq!(stats.entities_updated, 3);
        let summary = eng
            .store
            .pages
            .values()
            .find(|p| p.title == "摘要：Source Alias")
            .unwrap();
        assert!(summary.markdown.contains("[[MCP-协议]]"));
        assert_eq!(summary.markdown.matches("[[MCP-协议]]").count(), 1);
        assert!(summary.markdown.contains("[[RAG]]"));
        assert!(summary.markdown.contains("[[Ollama]]"));
        assert!(summary.markdown.contains("[[Dify]]"));
        assert!(summary.markdown.contains("[[Vane]]"));
        assert!(!summary.markdown.contains("[[MCP协议]]"));
        assert!(!summary
            .markdown
            .contains("[[MCP（Model Context Protocol）]]"));
        assert!(!summary.markdown.contains("[[ollama/ollama]]"));
        assert!(!eng.store.pages.values().any(|p| p.title == "MCP协议"));
        assert!(!eng
            .store
            .pages
            .values()
            .any(|p| p.title == "MCP（Model Context Protocol）"));
        assert!(!eng.store.pages.values().any(|p| p.title == "ollama/ollama"));
    }

    #[test]
    fn new_github_repo_entity_uses_repo_name_as_page_title() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let plan = plan_with_entities(vec![entity("danny-avila/LibreChat")]);
        let batch = BatchIngestContext {
            source_title: "Source Repo".into(),
            source_url: "https://example.test/repo".into(),
            source_tags: vec![],
        };

        let stats = materialize_compiler_pages(
            &mut eng,
            &plan,
            &batch,
            "https://example.test/repo",
            &test_scope(),
            &DomainSchema::permissive_default(),
        );

        assert_eq!(stats.entities_created, 1);
        assert!(eng.store.pages.values().any(|p| p.title == "LibreChat"));
        assert!(!eng
            .store
            .pages
            .values()
            .any(|p| p.title == "danny-avila/LibreChat"));
        let summary = eng
            .store
            .pages
            .values()
            .find(|p| p.title == "摘要：Source Repo")
            .unwrap();
        assert!(summary.markdown.contains("[[LibreChat]]"));
    }

    #[test]
    fn concept_draft_updates_existing_entity_instead_of_creating_cross_type_duplicate() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let existing = WikiPage::new("Claude Cowork", "# Claude Cowork\n", test_scope())
            .with_entry_type(EntryType::Entity)
            .with_status(EntryStatus::Draft);
        eng.store.pages.insert(existing.id, existing);
        let mut plan = plan_with_entities(Vec::new());
        plan.concepts = vec![concept("Claude Cowork")];
        let batch = BatchIngestContext {
            source_title: "Source Cowork".into(),
            source_url: "https://example.test/cowork".into(),
            source_tags: vec![],
        };

        let stats = materialize_compiler_pages(
            &mut eng,
            &plan,
            &batch,
            "https://example.test/cowork",
            &test_scope(),
            &DomainSchema::permissive_default(),
        );

        assert_eq!(stats.concepts_created, 0);
        assert_eq!(stats.entities_updated, 1);
        assert_eq!(
            eng.store
                .pages
                .values()
                .filter(|p| p.title == "Claude Cowork")
                .count(),
            1
        );
        assert!(!eng
            .store
            .pages
            .values()
            .any(|p| p.title == "Claude Cowork" && p.entry_type == Some(EntryType::Concept)));
        let summary = eng
            .store
            .pages
            .values()
            .find(|p| p.title == "摘要：Source Cowork")
            .unwrap();
        assert!(summary.markdown.contains("[[Claude Cowork]]"));
    }

    #[test]
    fn ambiguous_candidates_skip_without_duplicate_page() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        for title in ["MCP 协议", "MCP 连接器"] {
            let page = WikiPage::new(
                title,
                format!("# {title}\n\naliases: MCP connectors\n"),
                test_scope(),
            )
            .with_entry_type(EntryType::Concept)
            .with_status(EntryStatus::Draft);
            eng.store.pages.insert(page.id, page);
        }
        let mut plan = plan_with_entities(Vec::new());
        plan.concepts = vec![concept("MCP connectors")];
        let batch = BatchIngestContext {
            source_title: "Source Ambiguous".into(),
            source_url: "https://example.test/ambiguous".into(),
            source_tags: vec![],
        };

        let stats = materialize_compiler_pages(
            &mut eng,
            &plan,
            &batch,
            "https://example.test/ambiguous",
            &test_scope(),
            &DomainSchema::permissive_default(),
        );

        assert_eq!(stats.concepts_created, 0);
        assert_eq!(stats.concepts_updated, 0);
        assert!(stats.warnings.is_empty());
        assert_eq!(stats.deferred_resolutions.len(), 1);
        assert_eq!(stats.deferred_resolutions[0].draft_title, "MCP connectors");
        assert_eq!(
            stats.deferred_resolutions[0].next_step,
            "machine_resolution"
        );
        assert!(!eng
            .store
            .pages
            .values()
            .any(|p| p.title == "MCP connectors"));
        let summary = eng
            .store
            .pages
            .values()
            .find(|p| p.title == "摘要：Source Ambiguous")
            .unwrap();
        assert!(!summary.markdown.contains("[[MCP connectors]]"));
    }

    #[test]
    fn candidate_retrieval_is_bounded_top_k() {
        let mut pages = HashMap::new();
        for i in 0..8 {
            let page = WikiPage::new(
                format!("Candidate {i}"),
                "# Candidate\n\naliases: shared alias\n",
                test_scope(),
            )
            .with_entry_type(EntryType::Concept)
            .with_status(EntryStatus::Draft);
            pages.insert(page.id, page);
        }
        let resolver = CompilerResolver::new(&pages, &test_scope());
        let item = DraftResolverItem {
            title: "shared alias".into(),
            entry_type: EntryType::Concept,
            body_hint: String::new(),
        };
        let keys = dedup_keys_for_title(&item.title);

        let candidates = resolver.retrieve_candidates(&item, &keys);

        assert_eq!(candidates.len(), COMPILER_RESOLVER_TOP_K);
    }

    #[test]
    fn candidate_retrieval_ignores_type_only_candidates() {
        let mut pages = HashMap::new();
        for title in ["/plan 规划模式", "1-of-1 DVN 配置", "1M 上下文"] {
            let page = WikiPage::new(title, format!("# {title}\n"), test_scope())
                .with_entry_type(EntryType::Concept)
                .with_status(EntryStatus::Draft);
            pages.insert(page.id, page);
        }
        let resolver = CompilerResolver::new(&pages, &test_scope());
        let item = DraftResolverItem {
            title: "备用机场策略".into(),
            entry_type: EntryType::Concept,
            body_hint: "主力机场不可用时使用备用节点和备用订阅的策略".into(),
        };
        let keys = dedup_keys_for_title(&item.title);

        let candidates = resolver.retrieve_candidates(&item, &keys);

        assert!(candidates.is_empty());
    }

    #[test]
    fn llm_fallback_parser_accepts_json_object_slice() {
        let parsed = parse_llm_fallback_decision(
            r#"```json
{"same_page":true,"canonical_title":"MCP 协议","confidence":"medium","reason":"same protocol"}
```"#,
        )
        .unwrap();

        assert!(parsed.same_page);
        assert_eq!(parsed.canonical_title, "MCP 协议");
        assert_eq!(parsed.confidence, ResolutionConfidence::Medium);
        assert_eq!(parsed.reason, "same protocol");
    }

    #[test]
    fn compiler_dedup_resolves_wechat_concept_aliases() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let existing_pages = vec![
            WikiPage::new("MCP 协议", "# MCP 协议\n", test_scope())
                .with_entry_type(EntryType::Concept)
                .with_status(EntryStatus::Draft),
            WikiPage::new("Agent Skills", "# Agent Skills\n", test_scope())
                .with_entry_type(EntryType::Concept)
                .with_status(EntryStatus::Draft),
            WikiPage::new("本地优先", "# 本地优先\n", test_scope())
                .with_entry_type(EntryType::Concept)
                .with_status(EntryStatus::Draft),
            WikiPage::new("品味", "# 品味\n", test_scope())
                .with_entry_type(EntryType::Concept)
                .with_status(EntryStatus::Draft),
        ];
        let mappings = vec![
            alias_mapping("MCP connectors", &existing_pages[0]),
            alias_mapping("MCP连接器", &existing_pages[0]),
            alias_mapping("Skills机制", &existing_pages[1]),
            alias_mapping("本地优先策略", &existing_pages[2]),
            alias_mapping("产品品味", &existing_pages[3]),
            alias_mapping("AI产品品味", &existing_pages[3]),
        ];
        for page in &existing_pages {
            eng.store.pages.insert(page.id, page.clone());
        }
        let mut plan = plan_with_entities(Vec::new());
        plan.concepts = vec![
            concept("MCP connectors"),
            concept("MCP连接器"),
            concept("Skills机制"),
            concept("本地优先策略"),
            concept("产品品味"),
            concept("AI产品品味"),
        ];
        let batch = BatchIngestContext {
            source_title: "Source WeChat".into(),
            source_url: "https://example.test/wechat".into(),
            source_tags: vec![],
        };

        let mut resolver =
            CompilerResolver::new(&eng.store.pages, &test_scope()).with_mappings(mappings);
        let stats = materialize_compiler_pages_with_resolver(
            &mut eng,
            &plan,
            &batch,
            "https://example.test/wechat",
            &test_scope(),
            &DomainSchema::permissive_default(),
            &mut resolver,
        );

        assert_eq!(stats.concepts_created, 0);
        assert_eq!(stats.concepts_updated, 4);
        for duplicate in [
            "MCP connectors",
            "MCP连接器",
            "Skills机制",
            "本地优先策略",
            "产品品味",
            "AI产品品味",
        ] {
            assert!(!eng.store.pages.values().any(|p| p.title == duplicate));
        }
        let summary = eng
            .store
            .pages
            .values()
            .find(|p| p.title == "摘要：Source WeChat")
            .unwrap();
        assert!(summary.markdown.contains("[[MCP 协议]]"));
        assert!(summary.markdown.contains("[[Agent Skills]]"));
        assert!(summary.markdown.contains("[[本地优先]]"));
        assert!(summary.markdown.contains("[[品味]]"));
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
    fn trusted_compiler_schema_allows_auto_fill_tags() {
        let mut schema = DomainSchema::permissive_default();
        schema.tag_config.max_new_tags_per_ingest = 1;
        schema.tag_config.deprecated_tags = vec!["MCP".into()];
        let trusted = trusted_compiler_schema(&schema);

        let source_tags = vec!["MCP".to_string(), "new-source".to_string()];
        let plan = LlmIngestPlanV1 {
            version: 1,
            summary: wiki_core::llm_ingest_plan::LlmSummaryDraft {
                tags: vec!["new-summary".into()],
                ..Default::default()
            },
            summary_title: "source".into(),
            summary_markdown: String::new(),
            one_sentence_summary: "one sentence".into(),
            key_insights: vec!["insight".into()],
            confidence: "medium".into(),
            tags: vec!["new-plan".into()],
            source_author: None,
            source_publisher: None,
            source_published_at: None,
            claims: Vec::new(),
            concepts: Vec::new(),
            entities: Vec::new(),
            relationships: Vec::new(),
        };

        preflight_llm_plan_tags(&plan, &source_tags, &trusted).unwrap();
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
