use clap::{Parser, Subcommand};
use std::path::PathBuf;
use time::OffsetDateTime;
use wiki_core::{
    document_visible_to_viewer, parse_memory_tier, ClaimId, DomainSchema, Entity, EntityId,
    EntityKind, LlmIngestPlanV1, MemoryTier, PageId, QueryContext, RelationKind, Scope,
    SessionCrystallizationInput, SourceId, TypedEdge, WikiPage,
};
use wiki_kernel::{
    format_claim_doc_id, merge_graph_rankings, write_lint_report, write_projection,
    InMemorySearchPorts, InMemoryStore, LlmWikiEngine, NoopWikiHook, SearchPorts,
};
use wiki_storage::{SqliteRepository, WikiRepository};
use wiki_mempalace_bridge::{consume_outbox_ndjson, MempalaceError, MempalaceWikiSink};

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
    },
    Lint,
    Promote {
        claim_id: String,
    },
    Crystallize {
        question: String,
        #[arg(long = "finding")]
        findings: Vec<String>,
        #[arg(long = "file")]
        files: Vec<String>,
        #[arg(long = "lesson")]
        lessons: Vec<String>,
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
    /// Run batch maintenance: confidence decay, lint, promote qualified claims.
    Maintenance,
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
    if !matches!(cli.cmd, Cmd::Mcp { .. }) {
        banner::print_startup_banner();
    }
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
        } => {
            let cfg = llm::load_llm_config(&cli.llm_config)?;
            let user = format!("Source URI:\n{uri}\n\nBody:\n{body}");
            let reply = llm::complete_chat(
                &cfg,
                llm::ingest_llm_system_prompt(),
                &user,
                1800,
            )?;
            let slice = llm::parse_json_object_slice(&reply);
            let plan: LlmIngestPlanV1 = serde_json::from_str(slice)
                .map_err(|e| format!("ingest-llm JSON parse error: {e}; raw={reply}"))?;
            if dry_run {
                println!("{}", serde_json::to_string_pretty(&plan)?);
                return Ok(());
            }
            let sc = parse_scope(&scope);
            let sid = eng.ingest_raw(uri, &body, sc.clone(), "cli");
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            if cli.vectors {
                let app = llm::load_app_config(&cli.llm_config)?;
                let body_short = truncate_chars(&body, 8000);
                let vec = llm::embed_first(&app, &body_short)?;
                repo.upsert_embedding(&format!("source:{}", sid.0), &vec)?;
            }
            for c in &plan.claims {
                let tier = parse_memory_tier(&c.tier).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
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
                eng.add_entity(entity)?;
            }
            for rd in &plan.relationships {
                let from_id = eng.store.entities.values()
                    .find(|e| e.label.eq_ignore_ascii_case(&rd.from_label))
                    .map(|e| e.id);
                let to_id = eng.store.entities.values()
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
                    eng.add_edge(edge)?;
                }
            }
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            if !plan.summary_markdown.trim().is_empty() {
                let title = if plan.summary_title.trim().is_empty() {
                    "ingest-summary".to_string()
                } else {
                    plan.summary_title.trim().to_string()
                };
                let page = WikiPage::new(title, plan.summary_markdown.clone(), sc.clone());
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
                let body_short = truncate_chars(&body, 8000);
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
                let page = query_to_page(&title, &query, &ranked, viewer.clone());
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
                let report = write_lint_report(root, &format!("lint-{}", timestamp_slug()), &findings)?;
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
        Cmd::Crystallize {
            question,
            findings,
            files,
            lessons,
        } => {
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
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!("page={} claims={}", draft.page.id.0, draft.claim_candidates.len());
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
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!("decay=applied lint_findings={} promoted={promoted}", findings.len());
        }
    }
    Ok(())
}

pub(crate) fn doc_id_visible_to_viewer(doc_id: &str, store: &InMemoryStore, viewer: &Scope) -> bool {
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

fn query_to_page(title: &str, query: &str, ranked: &[(String, f64)], scope: Scope) -> WikiPage {
    let mut md = format!("# {title}\n\n## Query\n\n{query}\n\n## Top Results\n\n");
    for (doc, score) in ranked.iter().take(20) {
        md.push_str(&format!("- `{doc}` score={score:.6}\n"));
    }
    WikiPage::new(title, md, scope)
}

fn read_graph_extras_lines(path: &PathBuf) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let s = std::fs::read_to_string(path)?;
    Ok(s
        .lines()
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
