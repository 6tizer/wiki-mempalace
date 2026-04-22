use clap::{Parser, Subcommand};
use std::path::PathBuf;
use time::OffsetDateTime;
use wiki_core::{
    document_visible_to_viewer, parse_memory_tier, ClaimId, DomainSchema, Entity, EntityId,
    EntityKind, EntryStatus, EntryType, LlmIngestPlanV1, MemoryTier, PageId, QueryContext,
    RelationKind, Scope, SessionCrystallizationInput, SourceId, TypedEdge, WikiPage,
};
use wiki_kernel::{
    format_claim_doc_id, initial_status_for, merge_graph_rankings, write_lint_report,
    write_projection, InMemorySearchPorts, InMemoryStore, LlmWikiEngine, NoopWikiHook, SearchPorts,
};
use wiki_mempalace_bridge::{consume_outbox_ndjson, MempalaceError, MempalaceWikiSink};
use wiki_storage::{SqliteRepository, WikiRepository};

mod banner;
mod llm;
mod mcp;

#[derive(Parser)]
#[command(name = "wiki")]
#[command(
    about = "SQLite + Markdown wiki, RRF query, NDJSON outbox; optional embeddings & MemPalace hooks.",
    long_about = None
)]
struct Cli {
    #[arg(long, default_value = "wiki.db")]
    db: PathBuf,
    #[arg(long)]
    schema: Option<PathBuf>,
    #[arg(long)]
    wiki_dir: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    sync_wiki: bool,
    /// 检索 / lint / promote 的视角 scope（多 agent 隔离）。例如 `private:cli` 或 `shared:team1`。
    #[arg(long, default_value = "private:cli")]
    viewer_scope: String,
    /// 使用 `llm-config.toml` 中 `[embed]` 做向量检索（需联网）。
    #[arg(long, default_value_t = false)]
    vectors: bool,
    #[arg(long, default_value = "llm-config.toml")]
    llm_config: PathBuf,
    /// 每行一个 `entity:` / `claim:` / `page:` doc id，与内核图路按轮次合并后作为 RRF 第三路。
    #[arg(long)]
    graph_extras_file: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Ingest {
        uri: String,
        body: String,
        #[arg(long, default_value = "private:cli")]
        scope: String,
    },
    IngestLlm {
        uri: String,
        body: String,
        #[arg(long, default_value = "private:cli")]
        scope: String,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// 为自动生成的 summary page 绑定 EntryType（如 concept、entity、qa）。
        #[arg(long)]
        entry_type: Option<String>,
    },
    FileClaim {
        text: String,
        #[arg(long, default_value = "private:cli")]
        scope: String,
        #[arg(long, default_value = "working")]
        tier: String,
    },
    SupersedeClaim {
        old_claim_id: String,
        new_text: String,
        #[arg(long, default_value = "private:cli")]
        scope: String,
        #[arg(long, default_value = "working")]
        tier: String,
    },
    Query {
        query: String,
        #[arg(long, default_value_t = 60.0)]
        rrf_k: f64,
        #[arg(long, default_value_t = 50)]
        per_stream_limit: usize,
        #[arg(long, default_value_t = false)]
        write_page: bool,
        #[arg(long)]
        page_title: Option<String>,
        /// 为 query 生成的 page 绑定 EntryType（如 concept、entity、qa）。
        #[arg(long)]
        entry_type: Option<String>,
    },
    Lint,
    Promote {
        claim_id: String,
    },
    /// Promote a page's lifecycle status (Draft → InReview → Approved).
    PromotePage {
        page_id: String,
        /// Target status. If omitted, auto-advance to the next status per lifecycle rule.
        #[arg(long)]
        to: Option<String>,
        /// Skip all promotion condition checks.
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    Crystallize {
        question: String,
        #[arg(long = "finding")]
        findings: Vec<String>,
        #[arg(long = "file")]
        files: Vec<String>,
        #[arg(long = "lesson")]
        lessons: Vec<String>,
        /// 为 crystallize 生成的 page 绑定 EntryType。
        #[arg(long)]
        entry_type: Option<String>,
    },
    ExportOutboxNdjson,
    ExportOutboxNdjsonFrom {
        #[arg(long, default_value_t = 0)]
        last_id: i64,
    },
    AckOutbox {
        #[arg(long)]
        up_to_id: i64,
        #[arg(long)]
        consumer_tag: String,
    },
    ConsumeToMempalace {
        #[arg(long, default_value_t = 0)]
        last_id: i64,
    },
    LlmSmoke {
        #[arg(long, default_value = "llm-config.toml")]
        config: PathBuf,
        #[arg(long, default_value = "Say 'ok' only.")]
        prompt: String,
    },
    /// Start a unified MCP server (wiki + mempalace) over stdin/stdout.
    Mcp {
        #[arg(long, default_value_t = false)]
        once: bool,
        #[arg(long)]
        palace: Option<String>,
    },
    /// Validate a DomainSchema JSON file and print summary.
    SchemaValidate {
        /// JSON 文件路径，默认 DomainSchema.json
        path: Option<PathBuf>,
    },
    /// Run batch maintenance: confidence decay, lint, promote qualified claims.
    Maintenance,
    /// 批量编译 vault 中 compiled_to_wiki: false 的 source 文件（调用 LLM 抽取后写入引擎）
    BatchIngest {
        /// vault 根目录（含 sources/）
        #[arg(long, default_value = "/Users/mac-mini/Documents/wiki")]
        vault: PathBuf,
        /// 限制处理条数（用于测试）
        #[arg(long)]
        limit: Option<usize>,
        /// 只扫描不编译，输出待处理列表
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// 每条之间休眠秒数（避免 LLM 限流）
        #[arg(long, default_value_t = 1)]
        delay_secs: u64,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = match Cli::try_parse() {
        Ok(v) => v,
        Err(e) => {
            banner::print_startup_banner();
            e.print()?;
            std::process::exit(if e.use_stderr() { 2 } else { 0 });
        }
    };
    if !matches!(cli.cmd, Cmd::Mcp { .. } | Cmd::SchemaValidate { .. }) {
        banner::print_startup_banner();
    }

    // SchemaValidate 不需要 DB / engine，直接短路
    if let Cmd::SchemaValidate { path } = cli.cmd {
        let p = path.unwrap_or_else(|| PathBuf::from("DomainSchema.json"));
        match DomainSchema::from_json_path(&p) {
            Ok(schema) => {
                println!(
                    "schema ok: title={} lifecycle_rules={}",
                    schema.title,
                    schema.lifecycle_rules.len()
                );
                Ok(())
            }
            Err(e) => {
                eprintln!("schema invalid: {e}");
                std::process::exit(1);
            }
        }
    } else {
        run_with_engine(cli)
    }
}

/// 所有需要 DB / engine 的子命令走这里。
fn run_with_engine(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let viewer = parse_scope(&cli.viewer_scope);
    let wiki_root = cli.wiki_dir.clone();
    let sync_wiki = cli.sync_wiki;
    let repo = SqliteRepository::open(&cli.db)?;
    let schema = if let Some(path) = &cli.schema {
        DomainSchema::from_json_path(path)?
    } else {
        DomainSchema::permissive_default()
    };
    let mut eng = LlmWikiEngine::load_from_repo(schema.clone(), &repo, NoopWikiHook)?;

    match cli.cmd {
        Cmd::IngestLlm {
            uri,
            body,
            scope,
            dry_run,
            entry_type: _entry_type,
        } => {
            let cfg = llm::load_llm_config(&cli.llm_config)?;
            let user = format!("Source URI:\n{uri}\n\nBody:\n{body}");
            let reply = llm::complete_chat(&cfg, llm::ingest_llm_system_prompt(), &user, 8192)?;
            let slice = llm::parse_json_object_slice(&reply);
            let plan: LlmIngestPlanV1 = serde_json::from_str(slice)
                .map_err(|e| format!("ingest-llm JSON parse error: {e}; raw={reply}"))?;
            if dry_run {
                println!("{}", serde_json::to_string_pretty(&plan)?);
                return Ok(());
            }
            let sc = parse_scope(&scope);
            let sid = eng.ingest_raw(uri.clone(), &body, sc.clone(), "cli");
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            if cli.vectors {
                let app = llm::load_app_config(&cli.llm_config)?;
                let body_short = truncate_chars(&body, 16000);
                let vec = llm::embed_first(&app, &body_short)?;
                repo.upsert_embedding(&format!("source:{}", sid.0), &vec)?;
            }
            for c in &plan.claims {
                let tier = parse_memory_tier(&c.tier).unwrap_or(MemoryTier::Semantic);
                let cid = eng.file_claim(c.text.clone(), sc.clone(), tier, "cli");
                eng.attach_sources(cid, &[sid])?;
                eng.save_to_repo(&repo)?;
                eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
                if cli.vectors {
                    let app = llm::load_app_config(&cli.llm_config)?;
                    let vec = llm::embed_first(&app, &c.text)?;
                    repo.upsert_embedding(&format_claim_doc_id(cid), &vec)?;
                }
            }
            for ed in &plan.entities {
                let kind = EntityKind::parse(&ed.kind);
                let entity = Entity {
                    id: EntityId(uuid::Uuid::new_v4()),
                    kind,
                    label: ed.label.clone(),
                    scope: sc.clone(),
                };
                let _ = eng.add_entity(entity);
            }
            for rd in &plan.relationships {
                let from_id = eng
                    .store
                    .entities
                    .values()
                    .find(|e| e.label.eq_ignore_ascii_case(&rd.from_label))
                    .map(|e| e.id);
                let to_id = eng
                    .store
                    .entities
                    .values()
                    .find(|e| e.label.eq_ignore_ascii_case(&rd.to_label))
                    .map(|e| e.id);
                if let (Some(from), Some(to)) = (from_id, to_id) {
                    let rel = RelationKind::parse(&rd.relation);
                    let edge = TypedEdge {
                        from,
                        to,
                        relation: rel,
                        confidence: 0.7,
                        source_ids: vec![sid],
                    };
                    let _ = eng.add_edge(edge);
                }
            }
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            // summary 页固定为 vault 约定的 Summary 类型 + 五段正文（与 batch-ingest 对齐）
            if plan.should_materialize_summary_page() {
                let title = if plan.summary_title.trim().is_empty() {
                    "ingest-summary".to_string()
                } else {
                    plan.summary_title.trim().to_string()
                };
                let md = plan.to_five_section_summary_body(Some(&uri));
                let status = initial_status_for(Some(&EntryType::Summary), &schema);
                let page = WikiPage::new(title, md, sc.clone())
                    .with_entry_type(EntryType::Summary)
                    .with_status(status);
                eng.store.pages.insert(page.id, page);
                eng.save_to_repo(&repo)?;
                eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            }
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!("ingested source={}", sid.0);
        }
        Cmd::Ingest { uri, body, scope } => {
            let sid = eng.ingest_raw(uri, &body, parse_scope(&scope), "cli");
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            if cli.vectors {
                let app = llm::load_app_config(&cli.llm_config)?;
                let body_short = truncate_chars(&body, 16000);
                let vec = llm::embed_first(&app, &body_short)?;
                repo.upsert_embedding(&format!("source:{}", sid.0), &vec)?;
            }
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!("ingested source={}", sid.0);
        }
        Cmd::FileClaim { text, scope, tier } => {
            let tier = parse_tier(&tier)?;
            let cid = eng.file_claim(text, parse_scope(&scope), tier, "cli");
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            if cli.vectors {
                let app = llm::load_app_config(&cli.llm_config)?;
                let t = eng.store.claims[&cid].text.clone();
                let vec = llm::embed_first(&app, &t)?;
                repo.upsert_embedding(&format_claim_doc_id(cid), &vec)?;
            }
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!("claim_id={}", cid.0);
        }
        Cmd::SupersedeClaim {
            old_claim_id,
            new_text,
            scope,
            tier,
        } => {
            let old = wiki_core::ClaimId(uuid::Uuid::parse_str(&old_claim_id)?);
            let tier = parse_tier(&tier)?;
            let new_id = eng.supersede(old, new_text, parse_scope(&scope), tier, "cli")?;
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            if cli.vectors {
                let app = llm::load_app_config(&cli.llm_config)?;
                let t = eng.store.claims[&new_id].text.clone();
                let vec = llm::embed_first(&app, &t)?;
                repo.upsert_embedding(&format_claim_doc_id(new_id), &vec)?;
            }
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!("new_claim_id={}", new_id.0);
        }
        Cmd::Query {
            query,
            rrf_k,
            per_stream_limit,
            write_page,
            page_title,
            entry_type,
        } => {
            let ctx = QueryContext::new(&query)
                .with_rrf_k(rrf_k)
                .with_per_stream_limit(per_stream_limit)
                .with_viewer_scope(viewer.clone());
            let vec_override = if cli.vectors {
                let app = llm::load_app_config(&cli.llm_config)?;
                let qv = llm::embed_first(&app, &query)?;
                let raw = repo.search_embeddings_cosine(&qv, per_stream_limit.saturating_mul(8))?;
                let ids: Vec<String> = raw
                    .into_iter()
                    .filter(|(id, _)| doc_id_visible_to_viewer(id, &eng.store, &viewer))
                    .map(|(id, _)| id)
                    .take(per_stream_limit)
                    .collect();
                if ids.is_empty() {
                    None
                } else {
                    Some(ids)
                }
            } else {
                None
            };
            let graph_override = if let Some(ref path) = cli.graph_extras_file {
                let extras = read_graph_extras_lines(path)?;
                let ports = InMemorySearchPorts::new(&eng.store, Some(viewer.clone()));
                let kernel = SearchPorts::graph_ranked_ids(&ports, &query, per_stream_limit);
                Some(merge_graph_rankings(kernel, extras, per_stream_limit))
            } else {
                None
            };
            let ranked = eng.query_pipeline_memory(
                &ctx,
                OffsetDateTime::now_utc(),
                "cli",
                vec_override,
                graph_override,
            );
            if write_page {
                let title = page_title.unwrap_or_else(|| format!("query-{}", timestamp_slug()));
                let page = query_to_page(
                    &title,
                    &query,
                    &ranked,
                    viewer.clone(),
                    parse_entry_type_opt(&entry_type)?,
                    &schema,
                );
                eng.store.pages.insert(page.id, page);
            }
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            for (id, score) in ranked.into_iter().take(20) {
                println!("{score:.6}\t{id}");
            }
        }
        Cmd::Lint => {
            let findings = eng.run_basic_lint("cli", Some(&viewer));
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            if let Some(root) = wiki_root.as_deref() {
                let report =
                    write_lint_report(root, &format!("lint-{}", timestamp_slug()), &findings)?;
                println!("lint_report={}", report.display());
            }
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            for f in findings {
                println!("{:?}\t{}\t{}", f.severity, f.code, f.message);
            }
        }
        Cmd::Promote { claim_id } => {
            let cid = wiki_core::ClaimId(uuid::Uuid::parse_str(&claim_id)?);
            eng.promote_if_qualified(cid, "cli", &viewer)?;
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!("promoted {claim_id}");
        }
        Cmd::PromotePage { page_id, to, force } => {
            let pid = wiki_core::PageId(uuid::Uuid::parse_str(&page_id)?);
            // 解析目标状态：未指定时按 rule 自动取下一跳
            let to_status = match to {
                Some(s) => wiki_core::EntryStatus::parse(&s)
                    .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?,
                None => {
                    // 查找当前 page 的 entry_type → rule → 找 from == page.status 的第一条 promotion
                    let page = eng.store.pages.get(&pid).ok_or("page not found")?;
                    let et = page.entry_type.as_ref().ok_or("page has no entry_type")?;
                    let rule = eng
                        .schema
                        .find_lifecycle_rule(et)
                        .ok_or("no lifecycle rule")?;
                    let promo = rule
                        .promotions
                        .iter()
                        .find(|p| p.from_status == page.status)
                        .ok_or("no next promotion available")?;
                    promo.to_status
                }
            };
            let now = OffsetDateTime::now_utc();
            eng.promote_page(pid, to_status, "cli", now, force)?;
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!("promoted page {page_id} to {to_status:?}");
        }
        Cmd::Crystallize {
            question,
            findings,
            files,
            lessons,
            entry_type,
        } => {
            let et = parse_entry_type_opt(&entry_type)?;
            let draft = eng.crystallize(
                SessionCrystallizationInput {
                    question,
                    findings,
                    files_touched: files,
                    lessons,
                    scope: Scope::Private {
                        agent_id: "cli".into(),
                    },
                },
                "cli",
            )?;
            // crystallize 内部已经 insert page，此处覆盖 entry_type 和 status
            let status = initial_status_for(et.as_ref(), &schema);
            if let Some(page) = eng.store.pages.get_mut(&draft.page.id) {
                if let Some(et) = et {
                    page.entry_type = Some(et);
                }
                page.status = status;
            }
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!(
                "page={} claims={}",
                draft.page.id.0,
                draft.claim_candidates.len()
            );
        }
        Cmd::ExportOutboxNdjson => {
            print!("{}", repo.export_outbox_ndjson()?);
        }
        Cmd::ExportOutboxNdjsonFrom { last_id } => {
            print!("{}", repo.export_outbox_ndjson_from_id(last_id)?);
        }
        Cmd::AckOutbox {
            up_to_id,
            consumer_tag,
        } => {
            let n = repo.mark_outbox_processed(up_to_id, &consumer_tag)?;
            println!("acked={n}");
        }
        Cmd::ConsumeToMempalace { last_id } => {
            let ndjson = repo.export_outbox_ndjson_from_id(last_id)?;
            let n = consume_outbox_ndjson(&CliMempalaceSink, &ndjson)?;
            println!("consumed={n}");
        }
        Cmd::LlmSmoke { config, prompt } => {
            let cfg = llm::load_llm_config(&config)?;
            let out = llm::smoke_chat_completion(&cfg, &prompt)?;
            println!("{out}");
        }
        Cmd::Mcp { once, palace } => {
            mcp::run_mcp(
                &cli.db,
                schema,
                &cli.viewer_scope,
                once,
                &cli.llm_config,
                cli.vectors,
                wiki_root.as_deref(),
                palace.as_deref(),
            )?;
        }
        Cmd::Maintenance => {
            let now = OffsetDateTime::now_utc();
            eng.apply_confidence_decay_all(now, 30.0);
            let findings = eng.run_basic_lint("cli", Some(&viewer));
            let mut promoted = 0u32;
            let claim_ids: Vec<ClaimId> = eng.store.claims.keys().copied().collect();
            for cid in claim_ids {
                if eng.promote_if_qualified(cid, "cli", &viewer).is_ok() {
                    promoted += 1;
                }
            }
            let pages_marked = eng.mark_stale_pages(now);
            let pages_cleaned = eng.cleanup_expired_pages(now);
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!(
                "decay=applied lint_findings={} promoted={promoted} pages_marked_needs_update={pages_marked} pages_auto_cleaned={pages_cleaned}",
                findings.len()
            );
        }
        Cmd::BatchIngest {
            ref vault,
            limit,
            dry_run,
            delay_secs,
        } => {
            batch_ingest_cmd(
                &mut eng,
                &repo,
                &cli,
                &vault,
                limit,
                dry_run,
                delay_secs,
                sync_wiki,
                wiki_root.as_deref(),
                &schema,
            )?;
        }
        // SchemaValidate 已在 main() 中短路，此处不可达
        Cmd::SchemaValidate { .. } => unreachable!(),
    }
    Ok(())
}

pub(crate) fn doc_id_visible_to_viewer(
    doc_id: &str,
    store: &InMemoryStore,
    viewer: &Scope,
) -> bool {
    if let Some(rest) = doc_id.strip_prefix("claim:") {
        if let Ok(u) = uuid::Uuid::parse_str(rest) {
            return store
                .claims
                .get(&ClaimId(u))
                .map(|c| document_visible_to_viewer(&c.scope, viewer))
                .unwrap_or(false);
        }
        return false;
    }
    if let Some(rest) = doc_id.strip_prefix("page:") {
        if let Ok(u) = uuid::Uuid::parse_str(rest) {
            return store
                .pages
                .get(&PageId(u))
                .map(|p| document_visible_to_viewer(&p.scope, viewer))
                .unwrap_or(false);
        }
        return false;
    }
    if let Some(rest) = doc_id.strip_prefix("entity:") {
        if let Ok(u) = uuid::Uuid::parse_str(rest) {
            return store
                .entities
                .get(&EntityId(u))
                .map(|e| document_visible_to_viewer(&e.scope, viewer))
                .unwrap_or(false);
        }
        return false;
    }
    if let Some(rest) = doc_id.strip_prefix("source:") {
        if let Ok(u) = uuid::Uuid::parse_str(rest) {
            return store
                .sources
                .get(&SourceId(u))
                .map(|s| document_visible_to_viewer(&s.scope, viewer))
                .unwrap_or(false);
        }
    }
    false
}

fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

pub(crate) fn parse_scope(s: &str) -> Scope {
    if let Some(x) = s.strip_prefix("shared:") {
        Scope::Shared {
            team_id: x.to_string(),
        }
    } else if let Some(x) = s.strip_prefix("private:") {
        Scope::Private {
            agent_id: x.to_string(),
        }
    } else {
        Scope::Private {
            agent_id: s.to_string(),
        }
    }
}

pub(crate) fn parse_tier(s: &str) -> Result<MemoryTier, Box<dyn std::error::Error>> {
    let x = s.trim().to_ascii_lowercase();
    match x.as_str() {
        "working" => Ok(MemoryTier::Working),
        "episodic" => Ok(MemoryTier::Episodic),
        "semantic" => Ok(MemoryTier::Semantic),
        "procedural" => Ok(MemoryTier::Procedural),
        _ => Err(format!("unknown tier: {s}").into()),
    }
}

/// 解析可选的 --entry-type 参数，使用 schema 的 strict parse。
pub(crate) fn parse_entry_type_opt(
    s: &Option<String>,
) -> Result<Option<EntryType>, Box<dyn std::error::Error>> {
    match s {
        Some(raw) => {
            let et = EntryType::parse(raw)
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            Ok(Some(et))
        }
        None => Ok(None),
    }
}

/// ingest-llm 场景下的 entry_type 缺省策略：未指定时回退为 Concept。
/// 其它入口（crystallize / draft-from-query）保留 None 语义以避免意外写死。
#[allow(dead_code)]
pub(crate) fn effective_ingest_entry_type(explicit: Option<EntryType>) -> EntryType {
    explicit.unwrap_or(EntryType::Concept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_entry_type_defaults_to_concept() {
        assert_eq!(effective_ingest_entry_type(None), EntryType::Concept);
    }

    #[test]
    fn effective_entry_type_preserves_explicit() {
        assert_eq!(
            effective_ingest_entry_type(Some(EntryType::Entity)),
            EntryType::Entity
        );
        assert_eq!(
            effective_ingest_entry_type(Some(EntryType::Synthesis)),
            EntryType::Synthesis
        );
    }
}

fn maybe_sync_projection(
    sync_wiki: bool,
    wiki_root: Option<&std::path::Path>,
    eng: &LlmWikiEngine<NoopWikiHook>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !sync_wiki {
        return Ok(());
    }
    if let Some(root) = wiki_root {
        let stats = write_projection(root, &eng.store, &eng.audits)?;
        println!(
            "projection pages={} claims={} sources={}",
            stats.pages_written, stats.claims_written, stats.sources_written
        );
    }
    Ok(())
}

fn query_to_page(
    title: &str,
    query: &str,
    ranked: &[(String, f64)],
    scope: Scope,
    entry_type: Option<EntryType>,
    schema: &DomainSchema,
) -> WikiPage {
    let mut md = format!("# {title}\n\n## Query\n\n{query}\n\n## Top Results\n\n");
    for (doc, score) in ranked.iter().take(20) {
        md.push_str(&format!("- `{doc}` score={score:.6}\n"));
    }
    let status = initial_status_for(entry_type.as_ref(), schema);
    let page = WikiPage::new(title, md, scope).with_status(status);
    match entry_type {
        Some(et) => page.with_entry_type(et),
        None => page,
    }
}

fn read_graph_extras_lines(path: &PathBuf) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let s = std::fs::read_to_string(path)?;
    Ok(s.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect())
}

fn timestamp_slug() -> String {
    let now = OffsetDateTime::now_utc();
    now.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "now".to_string())
        .replace(':', "-")
}

struct CliMempalaceSink;

impl MempalaceWikiSink for CliMempalaceSink {
    fn on_claim_upserted(&self, _claim: &wiki_core::Claim) -> Result<(), MempalaceError> {
        Ok(())
    }

    fn on_claim_event(&self, claim_id: wiki_core::ClaimId) -> Result<(), MempalaceError> {
        println!("mempalace claim_upserted {}", claim_id.0);
        Ok(())
    }

    fn on_claim_superseded(
        &self,
        old: wiki_core::ClaimId,
        new: wiki_core::ClaimId,
    ) -> Result<(), MempalaceError> {
        println!("mempalace claim_superseded {} -> {}", old.0, new.0);
        Ok(())
    }

    fn on_source_linked(
        &self,
        source_id: wiki_core::SourceId,
        claim_id: wiki_core::ClaimId,
    ) -> Result<(), MempalaceError> {
        println!("mempalace source_linked {} -> {}", source_id.0, claim_id.0);
        Ok(())
    }

    fn scope_filter(&self, _scope: &Scope) -> bool {
        true
    }

    fn on_source_ingested(&self, source_id: SourceId) -> Result<(), MempalaceError> {
        println!("mempalace source_ingested {}", source_id.0);
        Ok(())
    }
}

// ── batch-ingest 相关 ──

/// 一条 source 的扫描结果
struct SourceEntry {
    path: PathBuf,
    title: String,
    url: String,
    body: String,
    /// 来自 frontmatter `tags`（逗号分隔）
    source_tags: Vec<String>,
    created_at: String,
}

/// batch 单条写入 summary / 引擎时携带的 vault 元数据
struct BatchIngestContext {
    source_title: String,
    source_url: String,
}

/// 单条 source 编译结果
struct IngestOneStats {
    claims: usize,
    entities: usize,
    relationships: usize,
    source_id: String,
    /// 完整 LLM 计划（用于写 pages/summary 与调试）
    plan: LlmIngestPlanV1,
}

/// 解析 frontmatter 中的 tags 字符串（支持中英文逗号）
fn parse_tags_csv(raw: Option<&String>) -> Vec<String> {
    raw.map(|s| {
        s.split(|c: char| c == ',' || c == '，')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

/// YAML 双引号内转义（与 wiki-kernel 投影一致）
fn yaml_escape_vault(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// 输出 `name:\n  - "..."` 或 `name: []`
fn yaml_string_list_block(name: &str, items: &[String]) -> String {
    if items.is_empty() {
        format!("{name}: []\n")
    } else {
        let mut out = format!("{name}:\n");
        for it in items {
            out.push_str(&format!("  - \"{}\"\n", yaml_escape_vault(it)));
        }
        out
    }
}

fn entry_status_yaml(status: EntryStatus) -> &'static str {
    match status {
        EntryStatus::Draft => "draft",
        EntryStatus::InReview => "in_review",
        EntryStatus::Approved => "approved",
        EntryStatus::NeedsUpdate => "needs_update",
    }
}

/// 扫描 vault/sources/ 中 compiled_to_wiki: false 的 source 文件
fn scan_uncompiled_sources(
    vault: &std::path::Path,
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
        let source_tags = parse_tags_csv(fm.get("tags"));
        let created_at = fm.get("created_at").cloned().unwrap_or_default();

        let body = if let Some(caps) = fm_caps {
            let fm_end = caps.get(0).unwrap().end();
            content[fm_end..].trim().to_string()
        } else {
            content.trim().to_string()
        };

        if body.len() < 50 {
            eprintln!("  跳过（正文过短）：{}", title);
            continue;
        }

        entries.push(SourceEntry {
            path: path.to_path_buf(),
            title,
            url,
            body,
            source_tags,
            created_at,
        });
    }

    entries.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(entries)
}

/// 简易 YAML frontmatter key: value 解析
fn parse_frontmatter_kv(text: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
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

/// 编译单条 source：LLM 抽取 + 写入引擎
fn ingest_one_source(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    cfg: &llm::LlmConfig,
    uri: &str,
    body: &str,
    scope: &Scope,
    vectors: bool,
    llm_config_path: &std::path::Path,
    schema: &DomainSchema,
    batch: &BatchIngestContext,
) -> Result<IngestOneStats, Box<dyn std::error::Error>> {
    let user = format!("Source URI:\n{uri}\n\nBody:\n{body}");
    let reply = llm::complete_chat(cfg, llm::ingest_llm_system_prompt(), &user, 8192)?;
    let slice = llm::parse_json_object_slice(&reply);
    let plan: LlmIngestPlanV1 =
        serde_json::from_str(slice).map_err(|e| format!("JSON parse error: {e}; raw={reply}"))?;

    let sid = eng.ingest_raw(uri, body, scope.clone(), "batch-ingest");
    eng.save_to_repo(repo)?;
    eng.flush_outbox_to_repo_with_policy(repo, 128, 3)?;

    if vectors {
        let app = llm::load_app_config(llm_config_path)?;
        let body_short = truncate_chars(body, 16000);
        let vec = llm::embed_first(&app, &body_short)?;
        repo.upsert_embedding(&format!("source:{}", sid.0), &vec)?;
    }

    for c in &plan.claims {
        let tier = parse_memory_tier(&c.tier).unwrap_or(MemoryTier::Semantic);
        let cid = eng.file_claim(c.text.clone(), scope.clone(), tier, "batch-ingest");
        eng.attach_sources(cid, &[sid])?;
        eng.save_to_repo(repo)?;
        eng.flush_outbox_to_repo_with_policy(repo, 128, 3)?;
        if vectors {
            let app = llm::load_app_config(llm_config_path)?;
            let vec = llm::embed_first(&app, &c.text)?;
            repo.upsert_embedding(&format_claim_doc_id(cid), &vec)?;
        }
    }

    for ed in &plan.entities {
        let kind = EntityKind::parse(&ed.kind);
        let entity = Entity {
            id: EntityId(uuid::Uuid::new_v4()),
            kind,
            label: ed.label.clone(),
            scope: scope.clone(),
        };
        // schema 可能拒绝不在白名单的 kind，跳过即可
        let _ = eng.add_entity(entity);
    }

    for rd in &plan.relationships {
        let from_id = eng
            .store
            .entities
            .values()
            .find(|e| e.label.eq_ignore_ascii_case(&rd.from_label))
            .map(|e| e.id);
        let to_id = eng
            .store
            .entities
            .values()
            .find(|e| e.label.eq_ignore_ascii_case(&rd.to_label))
            .map(|e| e.id);
        if let (Some(from), Some(to)) = (from_id, to_id) {
            let rel = RelationKind::parse(&rd.relation);
            let edge = TypedEdge {
                from,
                to,
                relation: rel,
                confidence: 0.7,
                source_ids: vec![sid],
            };
            // schema 可能拒绝不在白名单的 relation，跳过即可
            let _ = eng.add_edge(edge);
        }
    }
    eng.save_to_repo(repo)?;
    eng.flush_outbox_to_repo_with_policy(repo, 128, 3)?;

    // summary 页：磁盘与引擎均约定为 EntryType::Summary + 五段正文
    if plan.should_materialize_summary_page() {
        let page_title = format!("摘要：{}", batch.source_title);
        let foot_url = if batch.source_url.trim().is_empty() {
            uri
        } else {
            batch.source_url.as_str()
        };
        let md = plan.to_five_section_summary_body(Some(foot_url));
        let et = EntryType::Summary;
        let status = initial_status_for(Some(&et), schema);
        let page = WikiPage::new(page_title, md, scope.clone())
            .with_entry_type(et)
            .with_status(status);
        eng.store.pages.insert(page.id, page);
        eng.save_to_repo(repo)?;
        eng.flush_outbox_to_repo_with_policy(repo, 128, 3)?;
    }

    Ok(IngestOneStats {
        claims: plan.claims.len(),
        entities: plan.entities.len(),
        relationships: plan.relationships.len(),
        source_id: sid.0.to_string(),
        plan: plan.clone(),
    })
}

/// 以 Notion / vault-standards 完整契约写 summary 页到 `pages/summary/`
fn write_batch_summary(
    wiki_root: &std::path::Path,
    source_title: &str,
    plan: &LlmIngestPlanV1,
    source_url: &str,
    source_tags: &[String],
    source_created_at: &str,
    schema: &DomainSchema,
) -> Result<(), Box<dyn std::error::Error>> {
    let summary_dir = wiki_root.join("pages").join("summary");
    std::fs::create_dir_all(&summary_dir)?;

    // 文件名：中文标题，仅将 `/` 替换为 `-`（与 docs/vault-standards.md 一致）
    let filename = format!("摘要：{}.md", source_title.replace('/', "-"));
    let path = summary_dir.join(&filename);

    let now = time::OffsetDateTime::now_utc();
    let now_str = now.format(&time::format_description::well_known::Rfc3339)?;
    let created_str = if source_created_at.trim().is_empty() {
        now_str.clone()
    } else {
        source_created_at.trim().to_string()
    };

    let status = initial_status_for(Some(&EntryType::Summary), schema);
    let status_s = entry_status_yaml(status);
    let conf = plan.normalized_summary_confidence();
    let foot = if source_url.trim().is_empty() {
        None
    } else {
        Some(source_url.trim())
    };
    let body_sections = plan.to_five_section_summary_body(foot);

    let title_esc = yaml_escape_vault(&format!("摘要：{source_title}"));
    let url_esc = yaml_escape_vault(source_url);

    let mut fm = String::from("---\n");
    fm.push_str(&format!("title: \"{title_esc}\"\n"));
    fm.push_str("entry_type: summary\n");
    fm.push_str(&format!("status: {status_s}\n"));
    fm.push_str(&format!("confidence: {conf}\n"));
    fm.push_str(&format!("source_url: \"{url_esc}\"\n"));
    fm.push_str(&yaml_string_list_block("source_tags", source_tags));
    fm.push_str(&yaml_string_list_block("tags", &plan.tags));
    fm.push_str(&format!(
        "created_at: \"{}\"\n",
        yaml_escape_vault(&created_str)
    ));
    fm.push_str(&format!(
        "updated_at: \"{}\"\n",
        yaml_escape_vault(&now_str)
    ));
    fm.push_str(&format!(
        "last_compiled_at: \"{}\"\n",
        yaml_escape_vault(&now_str)
    ));
    fm.push_str("compiled_by: batch-ingest\n");
    fm.push_str("---\n\n");

    let h1_esc = source_title.replace('/', "-");
    let content = format!(
        "{fm}# 摘要：{h1_esc}\n\n{body_sections}",
        fm = fm,
        h1_esc = h1_esc,
        body_sections = body_sections
    );

    std::fs::write(&path, content)?;
    Ok(())
}

/// batch-ingest 子命令入口
fn batch_ingest_cmd(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    cli: &Cli,
    vault: &std::path::Path,
    limit: Option<usize>,
    dry_run: bool,
    delay_secs: u64,
    _sync_wiki: bool,
    wiki_root: Option<&std::path::Path>,
    schema: &DomainSchema,
) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("扫描未编译 source...");
    let mut sources = scan_uncompiled_sources(vault)?;
    eprintln!("  → 找到 {} 条未编译 source", sources.len());

    if let Some(n) = limit {
        sources.truncate(n);
        eprintln!("  → --limit {}，处理前 {} 条", n, sources.len());
    }

    if dry_run {
        for (i, s) in sources.iter().enumerate() {
            println!(
                "{}/{}) {} ({} 字符)",
                i + 1,
                sources.len(),
                s.title,
                s.body.len()
            );
        }
        return Ok(());
    }

    let cfg = llm::load_llm_config(&cli.llm_config)?;
    let scope = parse_scope("private:batch-ingest");

    let mut ok_count = 0usize;
    let mut err_count = 0usize;

    for (i, src) in sources.iter().enumerate() {
        let uri = if src.url.is_empty() {
            format!("file://{}", src.path.display())
        } else {
            src.url.clone()
        };

        eprintln!("[{}/{}] {}...", i + 1, sources.len(), src.title);

        let batch_ctx = BatchIngestContext {
            source_title: src.title.clone(),
            source_url: src.url.clone(),
        };

        match ingest_one_source(
            eng,
            repo,
            &cfg,
            &uri,
            &src.body,
            &scope,
            cli.vectors,
            &cli.llm_config,
            schema,
            &batch_ctx,
        ) {
            Ok(stats) => {
                eprintln!(
                    "  ✓ claims={} entities={} rels={} source={}",
                    stats.claims, stats.entities, stats.relationships, stats.source_id,
                );

                // 更新 source .md 的 compiled_to_wiki 标记
                let content = std::fs::read_to_string(&src.path)?;
                let new_content =
                    content.replace("compiled_to_wiki: false", "compiled_to_wiki: true");
                if new_content != content {
                    std::fs::write(&src.path, new_content)?;
                }

                // 按 vault-standards 写 summary 页到 pages/summary/
                if wiki_root.is_some() && stats.plan.should_materialize_summary_page() {
                    write_batch_summary(
                        wiki_root.unwrap(),
                        &src.title,
                        &stats.plan,
                        &src.url,
                        &src.source_tags,
                        &src.created_at,
                        schema,
                    )?;
                }

                ok_count += 1;
            }
            Err(e) => {
                eprintln!("  ✗ 失败：{e}");
                err_count += 1;
            }
        }

        // 限流
        if i + 1 < sources.len() && delay_secs > 0 {
            std::thread::sleep(std::time::Duration::from_secs(delay_secs));
        }
    }

    eprintln!("\n完成：成功={ok_count} 失败={err_count}");
    Ok(())
}
