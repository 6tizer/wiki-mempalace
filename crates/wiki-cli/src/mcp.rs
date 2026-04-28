#![allow(clippy::too_many_arguments)]

use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use time::OffsetDateTime;
use wiki_core::{
    normalize_and_validate_tag_groups, parse_memory_tier, ClaimId, DomainSchema, EntryType,
    MemoryTier, QueryContext, Scope, SessionCrystallizationInput, WikiPage,
};
use wiki_kernel::{initial_status_for, write_projection, LlmWikiEngine, NoopWikiHook};
use wiki_storage::{EmbeddingWrite, SqliteRepository};

use crate::{parse_scope, parse_tier};

const MAX_MCP_LINE_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
enum McpToolError {
    #[error("{0}")]
    InvalidParams(String),
    #[error("{0}")]
    MethodNotFound(String),
    #[error("{0}")]
    ToolNotFound(String),
    #[error("{0}")]
    Engine(String),
    #[error("{0}")]
    Storage(String),
    #[error("{0}")]
    Llm(String),
    #[error("{0}")]
    Mempalace(String),
}

impl McpToolError {
    fn invalid_params(message: impl std::fmt::Display) -> Self {
        Self::InvalidParams(message.to_string())
    }

    fn method_not_found(method: &str) -> Self {
        Self::MethodNotFound(format!("unknown method: {method}"))
    }

    fn tool_not_found(tool: &str) -> Self {
        Self::ToolNotFound(format!("unknown tool: {tool}"))
    }

    fn engine(error: impl std::fmt::Display) -> Self {
        Self::Engine(error.to_string())
    }

    fn storage(error: impl std::fmt::Display) -> Self {
        Self::Storage(error.to_string())
    }

    fn llm(error: impl std::fmt::Display) -> Self {
        Self::Llm(error.to_string())
    }

    fn mempalace(error: impl std::fmt::Display) -> Self {
        Self::Mempalace(error.to_string())
    }

    fn json_rpc_code(&self) -> i64 {
        match self {
            Self::InvalidParams(_) | Self::ToolNotFound(_) => -32602,
            Self::MethodNotFound(_) => -32601,
            Self::Engine(_) | Self::Storage(_) | Self::Llm(_) | Self::Mempalace(_) => -32000,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::InvalidParams(_) => "invalid_params",
            Self::MethodNotFound(_) => "method_not_found",
            Self::ToolNotFound(_) => "tool_not_found",
            Self::Engine(_) => "engine_error",
            Self::Storage(_) => "storage_error",
            Self::Llm(_) => "llm_error",
            Self::Mempalace(_) => "mempalace_error",
        }
    }

    fn error_object(&self) -> Value {
        json!({
            "code": self.json_rpc_code(),
            "message": self.to_string(),
            "data": {"kind": self.kind()}
        })
    }
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, McpToolError> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| McpToolError::invalid_params(format!("missing {key}")))
}

fn parse_json_rpc_request_line(line: &str) -> Result<Value, serde_json::Error> {
    serde_json::from_str(line.trim())
}

fn json_rpc_parse_error_object(error: impl std::fmt::Display) -> Value {
    json!({
        "code": -32700,
        "message": error.to_string(),
        "data": {"kind": "parse_error"}
    })
}

pub fn run_mcp(
    db_path: &std::path::Path,
    schema: DomainSchema,
    viewer_scope: &str,
    once: bool,
    llm_config_path: &std::path::Path,
    vectors: bool,
    wiki_dir: Option<&std::path::Path>,
    palace_path: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = SqliteRepository::open(db_path)?;
    let mut eng = LlmWikiEngine::load_from_repo(schema, &repo, NoopWikiHook)?;
    let viewer = parse_scope(viewer_scope);

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut handle = stdin.lock();
    while let Some(line) = read_line_limited(&mut handle, MAX_MCP_LINE_BYTES)? {
        if line.is_empty() {
            break;
        }
        let req: Value = match parse_json_rpc_request_line(&line) {
            Ok(v) => v,
            Err(e) => {
                writeln!(
                    stdout,
                    "{}",
                    json!({
                        "jsonrpc":"2.0",
                        "id":Value::Null,
                        "error": json_rpc_parse_error_object(e)
                    })
                )?;
                stdout.flush()?;
                if once {
                    break;
                }
                continue;
            }
        };

        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(Value::as_str).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or_else(|| json!({}));

        let resp = handle_request(
            method,
            params,
            id,
            &mut eng,
            &repo,
            &viewer,
            llm_config_path,
            vectors,
            wiki_dir,
            palace_path,
        );
        writeln!(stdout, "{resp}")?;
        stdout.flush()?;
        if once {
            break;
        }
    }
    Ok(())
}

fn handle_request(
    method: &str,
    params: Value,
    id: Value,
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    viewer: &Scope,
    llm_config_path: &std::path::Path,
    vectors: bool,
    wiki_dir: Option<&std::path::Path>,
    palace_path: Option<&str>,
) -> Value {
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {"name": "llm-wiki-unified", "version": "0.1.0"},
            "capabilities": {"tools": {}}
        })),
        "tools/list" => Ok(tools_list()),
        "tools/call" => call_tool(
            params,
            eng,
            repo,
            viewer,
            llm_config_path,
            vectors,
            wiki_dir,
            palace_path,
        ),
        _ => Err(McpToolError::method_not_found(method)),
    };

    match result {
        Ok(v) => json!({"jsonrpc":"2.0","id":id,"result":v}),
        Err(e) => json!({"jsonrpc":"2.0","id":id,"error":e.error_object()}),
    }
}

fn tools_list() -> Value {
    json!({
        "tools": [
            // --- Wiki-native tools ---
            {
                "name": "wiki_status",
                "description": "Wiki knowledge base statistics: claims, pages, entities, sources, audit records",
                "inputSchema": {"type":"object","properties":{}}
            },
            {
                "name": "wiki_ingest",
                "description": "Ingest raw source text with automatic PII redaction",
                "inputSchema": {"type":"object","properties":{
                    "uri":{"type":"string","description":"Source URI"},
                    "body":{"type":"string","description":"Source text body"},
                    "scope":{"type":"string","description":"Scope: private:<agent> or shared:<team>. Defaults to server --viewer-scope."},
                    "tags":{"type":"array","items":{"type":"string"},"description":"Optional source tags"}
                },"required":["uri","body"]}
            },
            {
                "name": "wiki_file_claim",
                "description": "Create a new knowledge claim with tier and scope",
                "inputSchema": {"type":"object","properties":{
                    "text":{"type":"string","description":"Claim text"},
                    "scope":{"type":"string","description":"Scope: private:<agent> or shared:<team>. Defaults to server --viewer-scope."},
                    "tier":{"type":"string","description":"Memory tier: working|episodic|semantic|procedural"},
                    "tags":{"type":"array","items":{"type":"string"},"description":"Optional claim tags"}
                },"required":["text"]}
            },
            {
                "name": "wiki_supersede_claim",
                "description": "Supersede an old claim with new text; old is marked stale",
                "inputSchema": {"type":"object","properties":{
                    "old_claim_id":{"type":"string","description":"UUID of the old claim"},
                    "new_text":{"type":"string","description":"New claim text"},
                    "scope":{"type":"string","description":"Scope. Defaults to server --viewer-scope."},
                    "tier":{"type":"string","description":"Memory tier"}
                },"required":["old_claim_id","new_text"]}
            },
            {
                "name": "wiki_query",
                "description": "Hybrid three-way RRF search across claims, pages, and entities",
                "inputSchema": {"type":"object","properties":{
                    "query":{"type":"string","description":"Natural language query"},
                    "rrf_k":{"type":"number","description":"RRF constant k (default 60)"},
                    "per_stream_limit":{"type":"integer","description":"Max results per stream (default 50)"},
                    "write_page":{"type":"boolean","description":"Write results as wiki page"}
                },"required":["query"]}
            },
            {
                "name": "wiki_promote_claim",
                "description": "Promote a claim up the memory tier if qualified by schema thresholds",
                "inputSchema": {"type":"object","properties":{
                    "claim_id":{"type":"string","description":"UUID of the claim to promote"}
                },"required":["claim_id"]}
            },
            {
                "name": "wiki_crystallize",
                "description": "Distill an exploration session into a wiki page and candidate claims",
                "inputSchema": {"type":"object","properties":{
                    "question":{"type":"string","description":"The session's main question"},
                    "findings":{"type":"array","items":{"type":"string"},"description":"Key findings"},
                    "files":{"type":"array","items":{"type":"string"},"description":"Files touched"},
                    "lessons":{"type":"array","items":{"type":"string"},"description":"Lessons learned"},
                    "entry_type":{"type":"string","description":"Optional EntryType for the generated page (e.g. concept, entity, qa)"}
                },"required":["question"]}
            },
            {
                "name": "wiki_lint",
                "description": "Run health checks: broken wikilinks, orphan pages, stale claims, missing cross-refs",
                "inputSchema": {"type":"object","properties":{}}
            },
            {
                "name": "wiki_wake_up",
                "description": "Enhanced wake-up context: top semantic claims, recent crystallizations, active entities",
                "inputSchema": {"type":"object","properties":{
                    "max_claims":{"type":"integer","description":"Max claims to include (default 5)"}
                }}
            },
            {
                "name": "wiki_maintenance",
                "description": "Batch maintenance: apply confidence decay, run lint, promote qualified claims",
                "inputSchema": {"type":"object","properties":{}}
            },
            {
                "name": "wiki_export_graph_dot",
                "description": "Export entity graph in DOT format for visualization",
                "inputSchema": {"type":"object","properties":{}}
            },
            {
                "name": "wiki_ingest_llm",
                "description": "LLM-driven structured ingestion: extract claims + a five-section Summary page. Since M7 the generated summary page is always EntryType::Summary.",
                "inputSchema": {"type":"object","properties":{
                    "uri":{"type":"string","description":"Source URI"},
                    "body":{"type":"string","description":"Source text body"},
                    "scope":{"type":"string","description":"Scope. Defaults to server --viewer-scope."},
                    "dry_run":{"type":"boolean","description":"If true, return plan without committing"}
                },"required":["uri","body"]}
            },
            // --- Mempalace passthrough tools ---
            {
                "name": "mempalace_status",
                "description": "Palace overview: drawer/wing/tunnel/kg_fact counts",
                "inputSchema": {"type":"object","properties":{}}
            },
            {
                "name": "mempalace_search",
                "description": "FTS5 hybrid search with BM25, sparse vectors, and RRF",
                "inputSchema": {"type":"object","properties":{
                    "query":{"type":"string"},
                    "wing":{"type":"string"},
                    "hall":{"type":"string"},
                    "room":{"type":"string"},
                    "bank_id":{"type":"string"},
                    "limit":{"type":"integer"},
                    "explain":{"type":"boolean"}
                },"required":["query"]}
            },
            {
                "name": "mempalace_wake_up",
                "description": "Get L0 identity + L1 critical facts wake-up context",
                "inputSchema": {"type":"object","properties":{"wing":{"type":"string"},"bank_id":{"type":"string"}}}
            },
            {
                "name": "mempalace_taxonomy",
                "description": "Wing/hall/room tree with drawer counts",
                "inputSchema": {"type":"object","properties":{"bank_id":{"type":"string"}}}
            },
            {
                "name": "mempalace_traverse",
                "description": "Follow tunnels (explicit + implicit connections) from a room",
                "inputSchema": {"type":"object","properties":{
                    "wing":{"type":"string"},
                    "room":{"type":"string"},
                    "bank_id":{"type":"string"}
                },"required":["wing","room"]}
            },
            {
                "name": "mempalace_kg_query",
                "description": "Query active temporal KG facts for a subject",
                "inputSchema": {"type":"object","properties":{
                    "subject":{"type":"string"},
                    "as_of":{"type":"string"}
                },"required":["subject"]}
            },
            {
                "name": "mempalace_kg_timeline",
                "description": "Full temporal timeline for a subject",
                "inputSchema": {"type":"object","properties":{"subject":{"type":"string"}},"required":["subject"]}
            },
            {
                "name": "mempalace_kg_stats",
                "description": "Knowledge graph statistics",
                "inputSchema": {"type":"object","properties":{}}
            },
            {
                "name": "mempalace_reflect",
                "description": "RAG: search palace + LLM synthesis",
                "inputSchema": {"type":"object","properties":{
                    "query":{"type":"string"},
                    "search_limit":{"type":"integer"},
                    "bank_id":{"type":"string"}
                },"required":["query"]}
            },
            {
                "name": "mempalace_extract",
                "description": "LLM-based SPO triple extraction from text",
                "inputSchema": {"type":"object","properties":{
                    "text":{"type":"string"},
                    "drawer_id":{"type":"integer"}
                }}
            }
        ]
    })
}

fn call_tool(
    params: Value,
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    viewer: &Scope,
    llm_config_path: &std::path::Path,
    vectors: bool,
    wiki_dir: Option<&std::path::Path>,
    palace_path: Option<&str>,
) -> Result<Value, McpToolError> {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    match name {
        // ──── Wiki-native tools ────
        "wiki_status" => {
            let claims = eng.store.claims.len();
            let pages = eng.store.pages.len();
            let entities = eng.store.entities.len();
            let sources = eng.store.sources.len();
            let audits = eng.audits.len();
            Ok(json!({
                "claims": claims,
                "pages": pages,
                "entities": entities,
                "sources": sources,
                "audit_records": audits
            }))
        }
        "wiki_ingest" => {
            let uri = required_str(&args, "uri")?;
            let body = required_str(&args, "body")?;
            let scope = resolve_write_scope(&args, viewer);
            let tags = tags_arg_from_value(&args, "tags")?;
            let sid = eng
                .ingest_raw_with_tags(uri.to_string(), body, scope, "mcp", &tags)
                .map_err(McpToolError::engine)?;
            let embeddings = if vectors {
                vec![
                    build_source_embedding(llm_config_path, &sid.0.to_string(), body)
                        .map_err(McpToolError::llm)?,
                ]
            } else {
                Vec::new()
            };
            save_flush_embeddings_and_project(eng, repo, wiki_dir, embeddings)?;
            Ok(json!({"source_id": sid.0.to_string()}))
        }
        "wiki_file_claim" => {
            let text = required_str(&args, "text")?;
            let scope = resolve_write_scope(&args, viewer);
            let tier = args
                .get("tier")
                .and_then(Value::as_str)
                .unwrap_or("working");
            let tier = parse_tier(tier).map_err(McpToolError::invalid_params)?;
            let tags = tags_arg_from_value(&args, "tags")?;
            let cid = eng
                .file_claim_with_tags(text.to_string(), scope, tier, "mcp", &tags)
                .map_err(McpToolError::engine)?;
            let embeddings = if vectors {
                vec![
                    build_text_embedding(llm_config_path, format!("claim:{}", cid.0), text)
                        .map_err(McpToolError::llm)?,
                ]
            } else {
                Vec::new()
            };
            save_flush_embeddings_and_project(eng, repo, wiki_dir, embeddings)?;
            Ok(json!({"claim_id": cid.0.to_string()}))
        }
        "wiki_supersede_claim" => {
            let old_id_str = required_str(&args, "old_claim_id")?;
            let new_text = required_str(&args, "new_text")?;
            let scope = resolve_write_scope(&args, viewer);
            let tier = args
                .get("tier")
                .and_then(Value::as_str)
                .unwrap_or("working");
            let old =
                ClaimId(uuid::Uuid::parse_str(old_id_str).map_err(McpToolError::invalid_params)?);
            let tier = parse_tier(tier).map_err(McpToolError::invalid_params)?;
            let new_id = eng
                .supersede(old, new_text.to_string(), scope, tier, "mcp")
                .map_err(McpToolError::engine)?;
            let embeddings = if vectors {
                vec![
                    build_text_embedding(llm_config_path, format!("claim:{}", new_id.0), new_text)
                        .map_err(McpToolError::llm)?,
                ]
            } else {
                Vec::new()
            };
            save_flush_embeddings_and_project(eng, repo, wiki_dir, embeddings)?;
            Ok(json!({"new_claim_id": new_id.0.to_string()}))
        }
        "wiki_query" => {
            let query = required_str(&args, "query")?;
            let rrf_k = args.get("rrf_k").and_then(Value::as_f64).unwrap_or(60.0);
            let limit = args
                .get("per_stream_limit")
                .and_then(Value::as_u64)
                .unwrap_or(50) as usize;
            let ctx = QueryContext::new(query)
                .with_rrf_k(rrf_k)
                .with_per_stream_limit(limit)
                .with_viewer_scope(viewer.clone());
            let ranked =
                eng.query_pipeline_memory(&ctx, OffsetDateTime::now_utc(), "mcp", None, None);
            let write_page = args
                .get("write_page")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if write_page {
                let title = format!("mcp-query-{}", OffsetDateTime::now_utc().unix_timestamp());
                let mut md = format!("# {title}\n\n## Query\n\n{query}\n\n## Results\n\n");
                for (doc, score) in ranked.iter().take(20) {
                    md.push_str(&format!("- `{doc}` score={score:.6}\n"));
                }
                let page = WikiPage::new(title, md, viewer.clone());
                eng.store.pages.insert(page.id, page);
            }
            save_flush_and_project(eng, repo, wiki_dir)?;
            Ok(json!({
                "results": ranked.iter().take(20).map(|(id, score)| json!({
                    "doc_id": id,
                    "score": score
                })).collect::<Vec<_>>()
            }))
        }
        "wiki_promote_claim" => {
            let cid_str = required_str(&args, "claim_id")?;
            let cid =
                ClaimId(uuid::Uuid::parse_str(cid_str).map_err(McpToolError::invalid_params)?);
            eng.promote_if_qualified(cid, "mcp", viewer)
                .map_err(McpToolError::engine)?;
            save_flush_and_project(eng, repo, wiki_dir)?;
            let claim = eng.store.claims.get(&cid);
            Ok(json!({
                "claim_id": cid_str,
                "tier": claim.map(|c| format!("{:?}", c.tier))
            }))
        }
        "wiki_crystallize" => {
            let question = required_str(&args, "question")?;
            let findings: Vec<String> = args
                .get("findings")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default();
            let files: Vec<String> = args
                .get("files")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default();
            let lessons: Vec<String> = args
                .get("lessons")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default();
            let entry_type_raw = args.get("entry_type").and_then(Value::as_str);
            let entry_type = match entry_type_raw {
                Some(s) => Some(EntryType::parse(s).map_err(McpToolError::invalid_params)?),
                None => None,
            };
            let draft = eng
                .crystallize(
                    SessionCrystallizationInput {
                        question: question.to_string(),
                        findings,
                        files_touched: files,
                        lessons,
                        scope: viewer.clone(),
                    },
                    "mcp",
                )
                .map_err(McpToolError::engine)?;
            // crystallize 内部已经 insert page，此处覆盖 entry_type 和 status
            let status = initial_status_for(entry_type.as_ref(), &eng.schema);
            if let Some(page) = eng.store.pages.get_mut(&draft.page.id) {
                if let Some(et) = entry_type {
                    page.entry_type = Some(et);
                }
                if page.status != status {
                    page.status = status;
                    page.status_entered_at = Some(OffsetDateTime::now_utc());
                }
            }
            save_flush_and_project(eng, repo, wiki_dir)?;
            Ok(json!({
                "page_id": draft.page.id.0.to_string(),
                "page_title": draft.page.title,
                "claim_candidates": draft.claim_candidates.len()
            }))
        }
        "wiki_lint" => {
            let findings = eng.run_basic_lint("mcp", Some(viewer));
            save_flush_and_project(eng, repo, wiki_dir)?;
            Ok(json!({
                "findings": findings.iter().map(|f| json!({
                    "severity": format!("{:?}", f.severity),
                    "code": f.code,
                    "message": f.message,
                    "subject": f.subject
                })).collect::<Vec<_>>()
            }))
        }
        "wiki_wake_up" => {
            let max_claims = args.get("max_claims").and_then(Value::as_u64).unwrap_or(5) as usize;
            let mut context = String::new();

            context.push_str("# L2 Active Semantic Knowledge\n\n");
            let mut top_claims: Vec<_> = eng
                .store
                .claims
                .values()
                .filter(|c| {
                    !c.stale && matches!(c.tier, MemoryTier::Semantic | MemoryTier::Procedural)
                })
                .filter(|c| wiki_core::document_visible_to_viewer(&c.scope, viewer))
                .collect();
            top_claims.sort_by(|a, b| {
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for c in top_claims.iter().take(max_claims) {
                context.push_str(&format!(
                    "- [conf={:.2}, {:?}] {}\n",
                    c.confidence, c.tier, c.text
                ));
            }

            context.push_str("\n# L3 Active Context\n\n");
            let recent_pages: Vec<_> = eng
                .store
                .pages
                .values()
                .filter(|p| wiki_core::document_visible_to_viewer(&p.scope, viewer))
                .collect();
            if !recent_pages.is_empty() {
                context.push_str("## Recent Pages\n");
                for p in recent_pages.iter().take(3) {
                    context.push_str(&format!("- {}\n", p.title));
                }
            }

            let entity_count = eng
                .store
                .entities
                .values()
                .filter(|e| wiki_core::document_visible_to_viewer(&e.scope, viewer))
                .count();
            context.push_str(&format!(
                "\n## Knowledge Graph: {} entities, {} edges\n",
                entity_count,
                eng.store.edges.len()
            ));

            Ok(json!({"context": context}))
        }
        "wiki_maintenance" => {
            let now = OffsetDateTime::now_utc();
            eng.apply_confidence_decay_all(now, 30.0);
            let findings = eng.run_basic_lint("mcp", Some(viewer));
            let mut promoted = 0u32;
            let claim_ids: Vec<ClaimId> = eng.store.claims.keys().copied().collect();
            for cid in claim_ids {
                if eng.promote_if_qualified(cid, "mcp", viewer).is_ok() {
                    promoted += 1;
                }
            }
            save_flush_and_project(eng, repo, wiki_dir)?;
            Ok(json!({
                "decay_applied": true,
                "lint_findings": findings.len(),
                "claims_promoted": promoted
            }))
        }
        "wiki_export_graph_dot" => {
            let mut dot = String::from("digraph wiki {\n  rankdir=LR;\n");
            let visible_entity_ids: std::collections::HashSet<_> = eng
                .store
                .entities
                .values()
                .filter(|e| wiki_core::document_visible_to_viewer(&e.scope, viewer))
                .map(|e| e.id)
                .collect();
            for id in &visible_entity_ids {
                if let Some(e) = eng.store.entities.get(id) {
                    dot.push_str(&format!(
                        "  \"{}\" [label=\"{} ({:?})\"];\n",
                        e.id.0, e.label, e.kind
                    ));
                }
            }
            for edge in &eng.store.edges {
                if visible_entity_ids.contains(&edge.from) && visible_entity_ids.contains(&edge.to)
                {
                    dot.push_str(&format!(
                        "  \"{}\" -> \"{}\" [label=\"{:?}\"];\n",
                        edge.from.0, edge.to.0, edge.relation
                    ));
                }
            }
            dot.push_str("}\n");
            Ok(json!({"dot": dot}))
        }
        "wiki_ingest_llm" => {
            let uri = required_str(&args, "uri")?;
            let body = required_str(&args, "body")?;
            let scope = resolve_write_scope(&args, viewer);
            let dry_run = args
                .get("dry_run")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if args.get("entry_type").is_some() {
                eprintln!(
                    "warning: wiki_ingest_llm.entry_type is deprecated since M7 and is ignored; \
                     summary pages are fixed to EntryType::Summary."
                );
            }

            let cfg = crate::llm::load_llm_config(llm_config_path).map_err(McpToolError::llm)?;
            let user_msg = format!("Source URI:\n{uri}\n\nBody:\n{body}");
            let reply = crate::llm::complete_chat(
                &cfg,
                crate::llm::ingest_llm_system_prompt(),
                &user_msg,
                8192,
            )
            .map_err(McpToolError::llm)?;
            let slice = crate::llm::parse_json_object_slice(&reply);
            let plan: wiki_core::LlmIngestPlanV1 = serde_json::from_str(slice)
                .map_err(|e| McpToolError::llm(format!("JSON parse error: {e}")))?;
            plan.validate_bounds()
                .map_err(|e| McpToolError::llm(format!("ingest plan validation error: {e}")))?;
            if dry_run {
                return Ok(json!({"plan": serde_json::to_value(&plan).unwrap_or(Value::Null)}));
            }
            preflight_llm_plan_tags(&plan, &plan.tags, &eng.schema)
                .map_err(McpToolError::invalid_params)?;
            let sid = eng
                .ingest_raw_with_tags(
                    uri.to_string(),
                    body,
                    scope.clone(),
                    "mcp",
                    plan.tags.iter().map(String::as_str),
                )
                .map_err(McpToolError::engine)?;
            let app = if vectors {
                Some(crate::llm::load_app_config(llm_config_path).map_err(McpToolError::llm)?)
            } else {
                None
            };
            let mut embeddings = Vec::new();
            if let Some(app) = &app {
                let short: String = body.chars().take(16000).collect();
                let v = crate::llm::embed_first(app, &short).map_err(McpToolError::llm)?;
                embeddings.push(EmbeddingWrite::new(format!("source:{}", sid.0), v));
            }
            for c in &plan.claims {
                let tier = parse_memory_tier(&c.tier).map_err(McpToolError::invalid_params)?;
                let cid = eng
                    .file_claim_with_tags(
                        c.text.clone(),
                        scope.clone(),
                        tier,
                        "mcp",
                        c.tags.iter().map(String::as_str),
                    )
                    .map_err(McpToolError::engine)?;
                eng.attach_sources(cid, &[sid])
                    .map_err(McpToolError::engine)?;
                if let Some(app) = &app {
                    let v = crate::llm::embed_first(app, &c.text).map_err(McpToolError::llm)?;
                    embeddings.push(EmbeddingWrite::new(format!("claim:{}", cid.0), v));
                }
            }
            // 生成 summary 页面：与 vault-standards / ingest-llm 一致（Summary + 五段正文）
            let mut summary_page_id: Option<String> = None;
            if plan.should_materialize_summary_page() {
                let title = if plan.summary_title.trim().is_empty() {
                    "ingest-summary".to_string()
                } else {
                    plan.summary_title.trim().to_string()
                };
                let md = plan.to_five_section_summary_body(Some(uri));
                let status = initial_status_for(Some(&EntryType::Summary), &eng.schema);
                let page = wiki_core::WikiPage::new(title, md, scope.clone())
                    .with_entry_type(EntryType::Summary)
                    .with_status(status);
                summary_page_id = Some(page.id.0.to_string());
                eng.store.pages.insert(page.id, page);
            }
            save_flush_embeddings_and_project(eng, repo, wiki_dir, embeddings)?;
            Ok(json!({
                "source_id": sid.0.to_string(),
                "claims_filed": plan.claims.len(),
                "summary": plan.summary_title,
                "summary_page_id": summary_page_id
            }))
        }

        // ──── Mempalace passthrough tools ────
        n if n.starts_with("mempalace_") => call_mempalace_tool(n, &args, palace_path),

        _ => Err(McpToolError::tool_not_found(name)),
    }
}

fn tags_arg_from_value(args: &Value, key: &str) -> Result<Vec<String>, McpToolError> {
    let Some(raw) = args.get(key) else {
        return Ok(Vec::new());
    };
    let arr = raw.as_array().ok_or_else(|| {
        McpToolError::invalid_params(format!("{key} must be an array of strings"))
    })?;
    arr.iter()
        .enumerate()
        .map(|(idx, v)| {
            v.as_str().map(ToString::to_string).ok_or_else(|| {
                McpToolError::invalid_params(format!("{key}[{idx}] must be a string"))
            })
        })
        .collect()
}

fn read_line_limited<R: BufRead>(reader: &mut R, max_bytes: usize) -> io::Result<Option<String>> {
    let mut buf = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            if buf.is_empty() {
                return Ok(None);
            }
            return String::from_utf8(buf)
                .map(Some)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e));
        }
        if let Some(pos) = available.iter().position(|b| *b == b'\n') {
            if buf.len() + pos + 1 > max_bytes {
                reader.consume(pos + 1);
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("MCP request line exceeds {max_bytes} bytes"),
                ));
            }
            buf.extend_from_slice(&available[..=pos]);
            reader.consume(pos + 1);
            return String::from_utf8(buf)
                .map(Some)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e));
        }
        if buf.len() + available.len() > max_bytes {
            let consume = max_bytes.saturating_sub(buf.len()).min(available.len());
            reader.consume(consume);
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("MCP request line exceeds {max_bytes} bytes"),
            ));
        }
        let len = available.len();
        buf.extend_from_slice(available);
        reader.consume(len);
    }
}

fn resolve_write_scope(args: &Value, viewer: &Scope) -> Scope {
    args.get("scope")
        .and_then(Value::as_str)
        .map(parse_scope)
        .unwrap_or_else(|| viewer.clone())
}

fn preflight_llm_plan_tags(
    plan: &wiki_core::LlmIngestPlanV1,
    source_tags: &[String],
    schema: &DomainSchema,
) -> Result<(), wiki_core::TagPolicyError> {
    let mut groups = Vec::with_capacity(plan.claims.len() + 1);
    groups.push(source_tags);
    groups.extend(plan.claims.iter().map(|claim| claim.tags.as_slice()));
    normalize_and_validate_tag_groups(&groups, schema)?;
    Ok(())
}

fn call_mempalace_tool(
    name: &str,
    args: &Value,
    palace_path: Option<&str>,
) -> Result<Value, McpToolError> {
    let tools = wiki_mempalace_bridge::make_tools(palace_path).map_err(McpToolError::mempalace)?;

    match name {
        "mempalace_status" => tools.status().map_err(McpToolError::mempalace),

        "mempalace_search" => {
            let query = required_str(args, "query")?;
            let wing = args.get("wing").and_then(Value::as_str);
            let hall = args.get("hall").and_then(Value::as_str);
            let room = args.get("room").and_then(Value::as_str);
            let bank_id = args.get("bank_id").and_then(Value::as_str);
            let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(8) as usize;
            let explain = args
                .get("explain")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            tools
                .search(query, wing, hall, room, bank_id, limit, explain)
                .map_err(McpToolError::mempalace)
        }

        "mempalace_wake_up" => {
            let wing = args.get("wing").and_then(Value::as_str);
            let bank_id = args.get("bank_id").and_then(Value::as_str);
            tools
                .wake_up(wing, bank_id)
                .map_err(McpToolError::mempalace)
        }

        "mempalace_taxonomy" => {
            let bank_id = args.get("bank_id").and_then(Value::as_str);
            tools.taxonomy(bank_id).map_err(McpToolError::mempalace)
        }

        "mempalace_traverse" => {
            let wing = required_str(args, "wing")?;
            let room = required_str(args, "room")?;
            let bank_id = args.get("bank_id").and_then(Value::as_str);
            tools
                .traverse(wing, room, bank_id)
                .map_err(McpToolError::mempalace)
        }

        "mempalace_kg_query" => {
            let subject = required_str(args, "subject")?;
            let as_of = args.get("as_of").and_then(Value::as_str);
            tools
                .kg_query(subject, as_of)
                .map_err(McpToolError::mempalace)
        }

        "mempalace_kg_timeline" => {
            let subject = required_str(args, "subject")?;
            tools.kg_timeline(subject).map_err(McpToolError::mempalace)
        }

        "mempalace_kg_stats" => tools.kg_stats().map_err(McpToolError::mempalace),

        "mempalace_reflect" => {
            let query = required_str(args, "query")?;
            let search_limit = args
                .get("search_limit")
                .and_then(Value::as_u64)
                .unwrap_or(8) as usize;
            let bank_id = args.get("bank_id").and_then(Value::as_str);
            tools
                .reflect(query, search_limit, bank_id)
                .map_err(McpToolError::mempalace)
        }

        "mempalace_extract" => {
            let text = args.get("text").and_then(Value::as_str);
            let drawer_id = args.get("drawer_id").and_then(Value::as_i64);
            tools
                .extract(text, drawer_id)
                .map_err(McpToolError::mempalace)
        }

        _ => Err(McpToolError::tool_not_found(name)),
    }
}

fn save_and_flush(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
) -> Result<(), Box<dyn std::error::Error>> {
    eng.save_to_repo_and_flush_outbox_with_policy(repo, 128, 3)?;
    Ok(())
}

fn save_flush_and_project(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    wiki_dir: Option<&std::path::Path>,
) -> Result<(), McpToolError> {
    save_and_flush(eng, repo).map_err(McpToolError::storage)?;
    if let Some(root) = wiki_dir {
        write_projection(root, &eng.store, &eng.audits).map_err(McpToolError::storage)?;
    }
    Ok(())
}

fn save_flush_embeddings_and_project(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    wiki_dir: Option<&std::path::Path>,
    embeddings: Vec<EmbeddingWrite>,
) -> Result<(), McpToolError> {
    if embeddings.is_empty() {
        return save_flush_and_project(eng, repo, wiki_dir);
    }
    let snapshot = eng.store.to_snapshot(&eng.audits);
    repo.save_snapshot_and_append_outbox_with_embeddings(&snapshot, &eng.outbox, &embeddings)
        .map_err(McpToolError::storage)?;
    eng.outbox.clear();
    if let Some(root) = wiki_dir {
        write_projection(root, &eng.store, &eng.audits).map_err(McpToolError::storage)?;
    }
    Ok(())
}

fn build_source_embedding(
    llm_config_path: &std::path::Path,
    source_id: &str,
    body: &str,
) -> Result<EmbeddingWrite, Box<dyn std::error::Error>> {
    let app = crate::llm::load_app_config(llm_config_path)?;
    let short: String = body.chars().take(16000).collect();
    let vec = crate::llm::embed_first(&app, &short)?;
    Ok(EmbeddingWrite::new(format!("source:{source_id}"), vec))
}

fn build_text_embedding(
    llm_config_path: &std::path::Path,
    doc_id: impl Into<String>,
    text: &str,
) -> Result<EmbeddingWrite, Box<dyn std::error::Error>> {
    let app = crate::llm::load_app_config(llm_config_path)?;
    let vec = crate::llm::embed_first(&app, text)?;
    Ok(EmbeddingWrite::new(doc_id, vec))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_list_wiki_ingest_llm_has_no_entry_type_param() {
        // M7 起 wiki_ingest_llm 产物固定 EntryType::Summary，inputSchema 不应再暴露 entry_type。
        let v = tools_list();
        let tools = v.get("tools").and_then(Value::as_array).expect("tools[]");
        let ingest = tools
            .iter()
            .find(|t| t.get("name").and_then(Value::as_str) == Some("wiki_ingest_llm"))
            .expect("wiki_ingest_llm 工具应存在");
        let props = ingest
            .pointer("/inputSchema/properties")
            .expect("inputSchema.properties");
        assert!(
            props.get("entry_type").is_none(),
            "entry_type 已在 M7 废弃，不应再出现在 tools/list 中"
        );
        // 保留核心参数契约
        for k in ["uri", "body", "scope", "dry_run"] {
            assert!(props.get(k).is_some(), "{k} 仍应存在");
        }
    }

    #[test]
    fn read_line_limited_rejects_oversized_line() {
        let mut cursor = std::io::Cursor::new(b"abcdef\n".as_slice());
        let err = read_line_limited(&mut cursor, 3).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn reliability_mcp_malformed_json_maps_to_parse_error() {
        let err = parse_json_rpc_request_line("{ not json").unwrap_err();
        let object = json_rpc_parse_error_object(&err);

        assert_eq!(object.get("code").and_then(Value::as_i64), Some(-32700));
        assert_eq!(
            object.pointer("/data/kind").and_then(Value::as_str),
            Some("parse_error")
        );
        assert_eq!(
            object.get("message").and_then(Value::as_str).unwrap(),
            err.to_string()
        );
    }

    #[test]
    fn mcp_error_object_maps_invalid_params() {
        let error = McpToolError::invalid_params("missing query");
        let object = error.error_object();

        assert_eq!(object.get("code").and_then(Value::as_i64), Some(-32602));
        assert_eq!(
            object.get("message").and_then(Value::as_str),
            Some("missing query")
        );
        assert_eq!(
            object.pointer("/data/kind").and_then(Value::as_str),
            Some("invalid_params")
        );
    }

    #[test]
    fn mcp_unknown_method_uses_json_rpc_method_not_found() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let repo = SqliteRepository::open(tmp.path().join("wiki.db")).expect("repo");
        let schema = DomainSchema::permissive_default();
        let mut eng = LlmWikiEngine::load_from_repo(schema, &repo, NoopWikiHook).expect("engine");
        let viewer = Scope::Shared {
            team_id: "wiki".into(),
        };

        let resp = handle_request(
            "not_a_method",
            json!({}),
            json!(1),
            &mut eng,
            &repo,
            &viewer,
            std::path::Path::new("llm-config.toml"),
            false,
            None,
            None,
        );

        assert_eq!(
            resp.pointer("/error/code").and_then(Value::as_i64),
            Some(-32601)
        );
        assert_eq!(
            resp.pointer("/error/data/kind").and_then(Value::as_str),
            Some("method_not_found")
        );
    }

    #[test]
    fn mcp_missing_tool_argument_uses_invalid_params_error() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let repo = SqliteRepository::open(tmp.path().join("wiki.db")).expect("repo");
        let schema = DomainSchema::permissive_default();
        let mut eng = LlmWikiEngine::load_from_repo(schema, &repo, NoopWikiHook).expect("engine");
        let viewer = Scope::Shared {
            team_id: "wiki".into(),
        };

        let resp = handle_request(
            "tools/call",
            json!({"name": "wiki_query", "arguments": {}}),
            json!(1),
            &mut eng,
            &repo,
            &viewer,
            std::path::Path::new("llm-config.toml"),
            false,
            None,
            None,
        );

        assert_eq!(
            resp.pointer("/error/code").and_then(Value::as_i64),
            Some(-32602)
        );
        assert_eq!(
            resp.pointer("/error/message").and_then(Value::as_str),
            Some("missing query")
        );
        assert_eq!(
            resp.pointer("/error/data/kind").and_then(Value::as_str),
            Some("invalid_params")
        );
    }

    #[test]
    fn mcp_unknown_tool_uses_tool_not_found_kind() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let repo = SqliteRepository::open(tmp.path().join("wiki.db")).expect("repo");
        let schema = DomainSchema::permissive_default();
        let mut eng = LlmWikiEngine::load_from_repo(schema, &repo, NoopWikiHook).expect("engine");
        let viewer = Scope::Shared {
            team_id: "wiki".into(),
        };

        let resp = handle_request(
            "tools/call",
            json!({"name": "not_a_tool", "arguments": {}}),
            json!(1),
            &mut eng,
            &repo,
            &viewer,
            std::path::Path::new("llm-config.toml"),
            false,
            None,
            None,
        );

        assert_eq!(
            resp.pointer("/error/code").and_then(Value::as_i64),
            Some(-32602)
        );
        assert_eq!(
            resp.pointer("/error/message").and_then(Value::as_str),
            Some("unknown tool: not_a_tool")
        );
        assert_eq!(
            resp.pointer("/error/data/kind").and_then(Value::as_str),
            Some("tool_not_found")
        );
    }

    #[test]
    fn mcp_write_projects_vault_when_wiki_dir_is_enabled() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let wiki_dir = tempfile::tempdir().expect("wiki dir");
        let repo = SqliteRepository::open(tmp.path().join("wiki.db")).expect("repo");
        let schema = DomainSchema::permissive_default();
        let mut eng = LlmWikiEngine::load_from_repo(schema, &repo, NoopWikiHook).expect("engine");
        let viewer = Scope::Shared {
            team_id: "wiki".into(),
        };

        let resp = handle_request(
            "tools/call",
            json!({
                "name": "wiki_query",
                "arguments": {
                    "query": "projection smoke",
                    "write_page": true
                }
            }),
            json!(1),
            &mut eng,
            &repo,
            &viewer,
            std::path::Path::new("llm-config.toml"),
            false,
            Some(wiki_dir.path()),
            None,
        );

        assert!(resp.get("error").is_none(), "{resp}");
        assert!(wiki_dir.path().join("index.md").exists());
        assert!(wiki_dir.path().join("log.md").exists());
        let page_dir = wiki_dir.path().join("pages/_unspecified");
        let page_count = std::fs::read_dir(page_dir)
            .expect("projection page dir")
            .count();
        assert_eq!(page_count, 1);
    }

    #[test]
    fn mcp_projection_failure_returns_storage_error_kind() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let wiki_root_file = tmp.path().join("not-a-dir");
        std::fs::write(&wiki_root_file, "not a directory").expect("sentinel file");
        let repo = SqliteRepository::open(tmp.path().join("wiki.db")).expect("repo");
        let schema = DomainSchema::permissive_default();
        let mut eng = LlmWikiEngine::load_from_repo(schema, &repo, NoopWikiHook).expect("engine");
        let viewer = Scope::Shared {
            team_id: "wiki".into(),
        };

        let resp = handle_request(
            "tools/call",
            json!({
                "name": "wiki_file_claim",
                "arguments": {
                    "text": "projection failure still reports typed storage error"
                }
            }),
            json!(1),
            &mut eng,
            &repo,
            &viewer,
            std::path::Path::new("llm-config.toml"),
            false,
            Some(&wiki_root_file),
            None,
        );

        assert_eq!(
            resp.pointer("/error/data/kind").and_then(Value::as_str),
            Some("storage_error")
        );
    }

    #[test]
    fn tag_tools_list_exposes_ingest_and_claim_tags() {
        let v = tools_list();
        let tools = v.get("tools").and_then(Value::as_array).expect("tools[]");

        for name in ["wiki_ingest", "wiki_file_claim"] {
            let tool = tools
                .iter()
                .find(|t| t.get("name").and_then(Value::as_str) == Some(name))
                .expect("tool should exist");
            let tags = tool
                .pointer("/inputSchema/properties/tags")
                .expect("tags property should exist");
            assert_eq!(tags.get("type").and_then(Value::as_str), Some("array"));
            assert_eq!(
                tags.pointer("/items/type").and_then(Value::as_str),
                Some("string")
            );
        }
    }

    #[test]
    fn tag_arg_from_value_parses_optional_string_array() {
        let args = json!({"tags": ["alpha", "beta"]});
        assert_eq!(
            tags_arg_from_value(&args, "tags").expect("tags should parse"),
            vec!["alpha".to_string(), "beta".to_string()]
        );

        assert_eq!(
            tags_arg_from_value(&json!({}), "tags").expect("missing tags should default"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn tag_arg_from_value_rejects_non_string_array_items() {
        let err = tags_arg_from_value(&json!({"tags": ["alpha", 42]}), "tags")
            .expect_err("non-string item should fail");

        assert_eq!(err.to_string(), "tags[1] must be a string");
        assert_eq!(err.kind(), "invalid_params");
    }

    #[test]
    fn mcp_write_scope_defaults_to_server_viewer_scope() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let repo = SqliteRepository::open(tmp.path().join("wiki.db")).expect("repo");
        let schema = DomainSchema::permissive_default();
        let mut eng = LlmWikiEngine::load_from_repo(schema, &repo, NoopWikiHook).expect("engine");
        let viewer = Scope::Shared {
            team_id: "wiki".into(),
        };
        let text = "b4 runtime default shared scope sentinel";

        let claim_resp = handle_request(
            "tools/call",
            json!({
                "name": "wiki_file_claim",
                "arguments": {
                    "text": text,
                    "tier": "semantic"
                }
            }),
            json!(1),
            &mut eng,
            &repo,
            &viewer,
            std::path::Path::new("llm-config.toml"),
            false,
            None,
            None,
        );
        assert!(claim_resp.get("error").is_none(), "{claim_resp}");
        let claim_id = claim_resp
            .pointer("/result/claim_id")
            .and_then(Value::as_str)
            .expect("claim_id");
        let claim_uuid = uuid::Uuid::parse_str(claim_id).expect("uuid");
        assert_eq!(
            eng.store.claims[&ClaimId(claim_uuid)].scope,
            Scope::Shared {
                team_id: "wiki".into()
            }
        );

        let query_resp = handle_request(
            "tools/call",
            json!({
                "name": "wiki_query",
                "arguments": {
                    "query": text,
                    "per_stream_limit": 10
                }
            }),
            json!(2),
            &mut eng,
            &repo,
            &viewer,
            std::path::Path::new("llm-config.toml"),
            false,
            None,
            None,
        );
        assert!(query_resp.get("error").is_none(), "{query_resp}");
        let expected_doc = format!("claim:{claim_id}");
        let visible = query_resp
            .pointer("/result/results")
            .and_then(Value::as_array)
            .expect("results")
            .iter()
            .any(|r| r.get("doc_id").and_then(Value::as_str) == Some(expected_doc.as_str()));
        assert!(
            visible,
            "shared:wiki query should see default-scoped write: {query_resp}"
        );
    }

    #[test]
    fn mcp_explicit_scope_overrides_server_viewer_scope() {
        let viewer = Scope::Shared {
            team_id: "wiki".into(),
        };

        assert_eq!(
            resolve_write_scope(&json!({}), &viewer),
            Scope::Shared {
                team_id: "wiki".into()
            }
        );
        assert_eq!(
            resolve_write_scope(&json!({"scope": "private:mcp"}), &viewer),
            Scope::Private {
                agent_id: "mcp".into()
            }
        );
    }

    #[test]
    fn tag_preflight_counts_source_and_claim_new_tags_per_ingest() {
        let mut schema = DomainSchema::permissive_default();
        schema.tag_config.max_new_tags_per_ingest = 1;
        let source_tags = vec!["new-source".to_string()];
        let plan = wiki_core::LlmIngestPlanV1 {
            version: 1,
            summary: wiki_core::llm_ingest_plan::LlmSummaryDraft::default(),
            summary_title: String::new(),
            summary_markdown: String::new(),
            one_sentence_summary: String::new(),
            key_insights: Vec::new(),
            confidence: String::new(),
            tags: Vec::new(),
            source_author: None,
            source_publisher: None,
            source_published_at: None,
            claims: vec![wiki_core::LlmClaimDraft {
                text: "claim".to_string(),
                tier: "semantic".to_string(),
                tags: vec!["new-claim".to_string()],
            }],
            concepts: Vec::new(),
            entities: Vec::new(),
            relationships: Vec::new(),
        };

        let err = preflight_llm_plan_tags(&plan, &source_tags, &schema).unwrap_err();

        assert_eq!(
            err,
            wiki_core::TagPolicyError::TooManyNewTags {
                count: 2,
                max: 1,
                tags: vec!["new-source".into(), "new-claim".into()],
            }
        );
    }
}
