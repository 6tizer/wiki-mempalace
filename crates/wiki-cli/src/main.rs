#![allow(clippy::items_after_test_module, clippy::too_many_arguments)]

use clap::Parser;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use wiki_core::{
    build_strategy_execution_plan, parse_memory_tier, AuditOperation, AuditRecord, Confidence,
    Entity, EntityId, EntityKind, EntryType, LlmIngestPlanV1, MemoryTier, PageContract,
    QueryContext, RelationKind, Scope, SessionCrystallizationInput, StrategyExecutionPlan,
    TypedEdge, WikiPage,
};
#[cfg(test)]
use wiki_core::{CompositeSearchPorts, FusionConfig};
#[cfg(test)]
use wiki_core::{EntryStatus, FixAction, FixActionType, FixPatch, WikiEvent};
#[cfg(test)]
use wiki_kernel::map_findings_to_fixes;
use wiki_kernel::{
    apply_evidence_fixer_plan, collect_wiki_metrics, discover_synthesis_candidates,
    finalize_consumed_page, format_claim_doc_id, initial_status_for,
    restore_evidence_fixer_tombstone, run_governance_scan, run_strategy_scan, write_projection,
    EvidenceFixerApplyOptions, GovernanceScanOptions, SearchPorts, StrategyScanOptions,
    SynthesisDiscoveryOptions,
};
#[cfg(test)]
use wiki_kernel::{InMemorySearchPorts, InMemoryStore, LlmWikiEngine, NoopWikiHook};
use wiki_mempalace_bridge::MempalaceSearchPorts;
#[cfg(test)]
use wiki_storage::SqliteRepository;
use wiki_storage::{canonical_notion_page_id, EmbeddingWrite, WikiRepository};

mod automation;
mod automation_jobs;
mod banner;
mod cli;
mod cli_utils;
mod commands;
mod compiler_deferred;
mod consistency;
mod dashboard;
mod governance;
mod llm;
mod mcp;
mod notion_archived_retirement;
mod notion_client;
mod notion_index_backfill;
mod notion_source_projection;
mod notion_sync;
mod notion_writeback;
mod orphan_governance;
mod palace_init;
mod research_synthesis;
mod strategy_render;
mod vault_audit;
mod vault_backfill;
mod web_search;
mod wiki_compiler;

use strategy_render::{
    parse_outbox_events, render_metrics_markdown, render_metrics_text,
    render_strategy_execution_plan_markdown, render_strategy_execution_plan_text,
    render_strategy_executor_apply_report_markdown, render_strategy_executor_apply_report_text,
    render_strategy_report_markdown, render_strategy_report_text, serialize_strategy_suggest_json,
    strategy_report_prefix,
};

use automation::{
    acquire_cli_writer_lease, automation_all_jobs, automation_health_level_name,
    automation_run_daily_jobs, collect_automation_health_report, collect_restore_verify_report,
    emit_automation_health_alert, format_automation_record, format_automation_time,
    format_outbox_consumer_progress, format_outbox_stats, print_automation_doctor,
    print_automation_jobs, print_automation_last_failures, print_automation_status,
    render_automation_health_report, render_restore_verify_report, run_automation_plan,
    run_verify_row_state, AutomationHealthLevel, AutomationHealthReport, AutomationHeartbeat,
};
#[cfg(test)]
use automation::{
    automation_health_thresholds, automation_job_name, automation_job_spec, automation_job_specs,
    classify_backlog, classify_consecutive_failures, classify_stale_heartbeat,
    prune_scheduled_report_runs, AutomationHealthIssue, AutomationHealthThresholds, AutomationJob,
};
#[cfg(test)]
use automation_jobs::{
    apply_auto_fixes, automation_notion_refresh_existing, gap_report_markdown, write_gap_report,
};
use automation_jobs::{
    apply_notion_sync_tag_policy, build_strategy_executor_apply_report, maybe_sync_projection,
    query_to_page, read_graph_extras_lines, run_consume_to_mempalace_job, run_daily_automation,
    run_fix_job, run_gap_job, run_lint_job, run_maintenance_job, run_notion_sync_cmd,
    run_research_synthesis_compose, run_single_automation_job,
    save_to_repo_and_flush_outbox_with_embeddings, EngineResolver, ResearchSynthesisComposeInputs,
};
#[cfg(test)]
use cli_utils::effective_ingest_entry_type;
use cli_utils::{
    default_dashboard_output, default_suggest_report_dir, ensure_parent_dir, parse_entry_type_opt,
    parse_scope, parse_tier, resolve_wiki_relative_path, timestamp_slug, truncate_chars,
    DEFAULT_MEMPALACE_CONSUMER_TAG,
};
use wiki_compiler::preflight_llm_plan_tags;
#[cfg(test)]
use wiki_compiler::{batch_source_tags_for_ingest, BatchIngestContext};

use cli::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = match Cli::try_parse() {
        Ok(v) => v,
        Err(e) => {
            banner::print_startup_banner();
            e.print()?;
            std::process::exit(if e.use_stderr() { 2 } else { 0 });
        }
    };
    if !matches!(
        &cli.cmd,
        Cmd::Mcp { .. }
            | Cmd::SchemaValidate { .. }
            | Cmd::Metrics { .. }
            | Cmd::Dashboard { .. }
            | Cmd::Suggest { .. }
            | Cmd::SuggestExecutorApply { .. }
            | Cmd::AiProfile { .. }
            | Cmd::WebSearch { .. }
            | Cmd::Governance { .. }
            | Cmd::ResearchSynthesis { .. }
            | Cmd::VerifyRowState { .. }
    ) {
        banner::print_startup_banner();
    }

    // SchemaValidate 不需要 DB / engine，直接短路
    if let Cmd::SchemaValidate { path } = cli.cmd {
        commands::schema::run(path)
    } else {
        run_with_engine(cli)
    }
}

/// 所有需要 DB / engine 的子命令走这里。
fn run_with_engine(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    if commands::dispatch::maybe_run_without_engine(&cli)? {
        return Ok(());
    }

    let commands::runtime::CliRuntime {
        viewer,
        wiki_root,
        sync_wiki,
        _writer_lease,
        repo,
        schema,
        mut eng,
    } = commands::runtime::open(&cli)?;

    match cli.cmd {
        Cmd::AiProfile { .. }
        | Cmd::WebSearch { .. }
        | Cmd::Governance {
            cmd: GovernanceCmd::FixerPlan { .. },
        }
        | Cmd::ResearchSynthesis {
            cmd: ResearchSynthesisCmd::Discover { scan: Some(_), .. },
        } => {
            unreachable!("no-engine command should be handled before opening wiki runtime")
        }
        Cmd::IngestLlm {
            uri,
            body,
            scope,
            dry_run,
            entry_type,
        } => {
            if entry_type.is_some() {
                eprintln!(
                    "warning: --entry-type on `ingest-llm` is deprecated since M7 and is ignored; \
                     all ingest-llm summary pages are fixed to EntryType::Summary."
                );
            }
            let cfg = llm::load_llm_config(&cli.llm_config)?;
            let user = llm::build_ingest_llm_user_prompt(&cfg, &uri, &body)?;
            let reply = llm::complete_chat_json_object(
                &cfg,
                llm::ingest_llm_system_prompt(),
                &user,
                cfg.max_output_tokens.min(8192),
            )?;
            let slice = llm::parse_json_object_slice(&reply);
            let plan: LlmIngestPlanV1 = serde_json::from_str(slice).map_err(|e| {
                format!(
                    "ingest-llm JSON parse error: {e}; raw={}",
                    llm::redact_for_llm_error(&reply)
                )
            })?;
            plan.validate_bounds()
                .map_err(|e| format!("ingest-llm plan validation error: {e}"))?;
            if dry_run {
                println!("{}", serde_json::to_string_pretty(&plan)?);
                return Ok(());
            }
            preflight_llm_plan_tags(&plan, &plan.tags, &schema)?;
            let sc = parse_scope(&scope);
            let sid = eng.ingest_raw_with_tags(
                uri.clone(),
                &body,
                sc.clone(),
                "cli",
                plan.tags.iter().map(String::as_str),
            )?;
            if cli.vectors {
                let app = llm::load_app_config(&cli.llm_config)?;
                let body_short = truncate_chars(&body, 16000);
                let vec = llm::embed_first(&app, &body_short)?;
                save_to_repo_and_flush_outbox_with_embeddings(
                    &mut eng,
                    &repo,
                    vec![EmbeddingWrite::new(format!("source:{}", sid.0), vec)],
                )?;
            } else {
                eng.save_to_repo_and_flush_outbox_with_policy(&repo, 128, 3)?;
            }
            for c in &plan.claims {
                let tier = parse_memory_tier(&c.tier).unwrap_or(MemoryTier::Semantic);
                let cid = eng.file_claim_with_tags(
                    c.text.clone(),
                    sc.clone(),
                    tier,
                    "cli",
                    c.tags.iter().map(String::as_str),
                )?;
                eng.attach_sources(cid, &[sid])?;
                if cli.vectors {
                    let app = llm::load_app_config(&cli.llm_config)?;
                    let vec = llm::embed_first(&app, &c.text)?;
                    save_to_repo_and_flush_outbox_with_embeddings(
                        &mut eng,
                        &repo,
                        vec![EmbeddingWrite::new(format_claim_doc_id(cid), vec)],
                    )?;
                } else {
                    eng.save_to_repo_and_flush_outbox_with_policy(&repo, 128, 3)?;
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
            eng.save_to_repo_and_flush_outbox_with_policy(&repo, 128, 3)?;
            // summary 页固定为 vault 约定的 Summary 类型 + 五段正文（与 batch-ingest 对齐）
            if plan.should_materialize_summary_page() {
                let title = if plan.summary_title.trim().is_empty() {
                    "ingest-summary".to_string()
                } else {
                    plan.summary_title.trim().to_string()
                };
                let md = plan.to_five_section_summary_body(Some(&uri));
                let page = WikiPage::new(title, md, sc.clone());
                let pid = page.id;
                eng.store.pages.insert(pid, page);
                if let Some(page) = eng.store.pages.get_mut(&pid) {
                    finalize_consumed_page(
                        page,
                        EntryType::Summary,
                        Confidence::default(),
                        &schema,
                    );
                }
                eng.save_to_repo_and_flush_outbox_with_policy(&repo, 128, 3)?;
            }
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!("ingested source={}", sid.0);
        }
        Cmd::Ingest {
            uri,
            body,
            scope,
            tags,
        } => {
            let sid = eng.ingest_raw_with_tags(uri, &body, parse_scope(&scope), "cli", &tags)?;
            if cli.vectors {
                let app = llm::load_app_config(&cli.llm_config)?;
                let body_short = truncate_chars(&body, 16000);
                let vec = llm::embed_first(&app, &body_short)?;
                save_to_repo_and_flush_outbox_with_embeddings(
                    &mut eng,
                    &repo,
                    vec![EmbeddingWrite::new(format!("source:{}", sid.0), vec)],
                )?;
            } else {
                eng.save_to_repo_and_flush_outbox_with_policy(&repo, 128, 3)?;
            }
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!("ingested source={}", sid.0);
        }
        Cmd::FileClaim {
            text,
            scope,
            tier,
            tags,
        } => {
            let tier = parse_tier(&tier)?;
            let cid = eng.file_claim_with_tags(text, parse_scope(&scope), tier, "cli", &tags)?;
            if cli.vectors {
                let app = llm::load_app_config(&cli.llm_config)?;
                let t = eng.store.claims[&cid].text.clone();
                let vec = llm::embed_first(&app, &t)?;
                save_to_repo_and_flush_outbox_with_embeddings(
                    &mut eng,
                    &repo,
                    vec![EmbeddingWrite::new(format_claim_doc_id(cid), vec)],
                )?;
            } else {
                eng.save_to_repo_and_flush_outbox_with_policy(&repo, 128, 3)?;
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
            if cli.vectors {
                let app = llm::load_app_config(&cli.llm_config)?;
                let t = eng.store.claims[&new_id].text.clone();
                let vec = llm::embed_first(&app, &t)?;
                save_to_repo_and_flush_outbox_with_embeddings(
                    &mut eng,
                    &repo,
                    vec![EmbeddingWrite::new(format_claim_doc_id(new_id), vec)],
                )?;
            } else {
                eng.save_to_repo_and_flush_outbox_with_policy(&repo, 128, 3)?;
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
            palace_db,
            palace_bank,
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
            let graph_extras = if let Some(ref path) = cli.graph_extras_file {
                let extras = read_graph_extras_lines(path)?;
                let extras = filter_graph_extras_for_viewer(extras, &eng.store, &viewer);
                Some(extras)
            } else {
                None
            };
            let ranked = run_fusion_query(
                palace_db.as_deref(),
                &palace_bank,
                &repo,
                &eng,
                &viewer,
                &ctx,
                OffsetDateTime::now_utc(),
                vec_override,
                graph_extras,
            );
            let top: Vec<String> = ranked.iter().take(24).map(|(id, _)| id.clone()).collect();
            eng.record_query(&query, Some(&viewer), top, "cli");
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
            eng.save_to_repo_and_flush_outbox_with_policy(&repo, 128, 3)?;
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            for (id, score) in ranked.into_iter().take(20) {
                println!("{score:.6}\t{id}");
            }
        }
        Cmd::Explain {
            query,
            rrf_k,
            per_stream_limit,
            palace_db,
            palace_bank,
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
            let graph_extras = if let Some(ref path) = cli.graph_extras_file {
                let extras = read_graph_extras_lines(path)?;
                let extras = filter_graph_extras_for_viewer(extras, &eng.store, &viewer);
                Some(extras)
            } else {
                None
            };

            println!("\n查询: \"{}\"", query);

            // wiki 各路结果
            let wiki_ports = build_wiki_search_ports(&repo, &eng, &viewer);
            let wiki_bm25 =
                SearchPorts::bm25_ranked_ids(wiki_ports.as_ref(), &query, per_stream_limit);
            let wiki_vector = vec_override.clone().unwrap_or_else(|| {
                SearchPorts::vector_ranked_ids(wiki_ports.as_ref(), &query, per_stream_limit)
            });
            let wiki_graph_base =
                SearchPorts::graph_ranked_ids(wiki_ports.as_ref(), &query, per_stream_limit);
            let wiki_graph = merge_optional_graph_extras(
                wiki_graph_base,
                graph_extras.clone(),
                per_stream_limit,
            );

            // mempalace 各路结果
            let mut mp_bm25: Vec<String> = Vec::new();
            let mut mp_vector: Vec<String> = Vec::new();
            let mut mp_graph: Vec<String> = Vec::new();
            let mut has_mp = false;
            if let Some(ref pdb) = palace_db {
                match MempalaceSearchPorts::open(Path::new(pdb), Some(palace_bank.to_string())) {
                    Ok(mp_ports) => {
                        mp_bm25 = SearchPorts::bm25_ranked_ids(&mp_ports, &query, per_stream_limit);
                        mp_vector =
                            SearchPorts::vector_ranked_ids(&mp_ports, &query, per_stream_limit);
                        mp_graph =
                            SearchPorts::graph_ranked_ids(&mp_ports, &query, per_stream_limit);
                        has_mp = true;
                    }
                    Err(e) => {
                        eprintln!(
                            "警告：无法打开 mempalace DB ({}): {}，跳过 mempalace 各路展示",
                            pdb, e
                        );
                    }
                }
            }

            let print_stream = |name: &str, ids: &[String]| {
                println!("{} ({}):", name, ids.len());
                for (i, id) in ids.iter().enumerate() {
                    println!("  #{} {}", i + 1, id);
                }
            };

            println!("\n=== BM25 路 ===");
            print_stream("wiki", &wiki_bm25);
            if has_mp {
                print_stream("mempalace", &mp_bm25);
            }

            println!("\n=== Vector 路 ===");
            if vec_override.is_some() {
                println!("wiki (override) ({}):", wiki_vector.len());
                for (i, id) in wiki_vector.iter().enumerate() {
                    println!("  #{} {}", i + 1, id);
                }
            } else {
                print_stream("wiki", &wiki_vector);
            }
            if has_mp {
                print_stream("mempalace", &mp_vector);
            }

            println!("\n=== Graph 路 ===");
            if graph_extras.is_some() {
                println!("wiki (override) ({}):", wiki_graph.len());
                for (i, id) in wiki_graph.iter().enumerate() {
                    println!("  #{} {}", i + 1, id);
                }
            } else {
                print_stream("wiki", &wiki_graph);
            }
            if has_mp {
                print_stream("mempalace", &mp_graph);
            }

            println!("\n=== RRF 融合结果 ===");
            let ranked = run_fusion_query(
                palace_db.as_deref(),
                &palace_bank,
                &repo,
                &eng,
                &viewer,
                &ctx,
                OffsetDateTime::now_utc(),
                vec_override,
                graph_extras,
            );
            for (i, (id, score)) in ranked.into_iter().take(20).enumerate() {
                println!("#{}: {:.6}  {}", i + 1, score, id);
            }
        }
        Cmd::Lint => {
            run_lint_job(&mut eng, &repo, &viewer, sync_wiki, wiki_root.as_deref())?;
        }
        Cmd::Gap {
            low_coverage_threshold,
            write_page,
        } => {
            run_gap_job(
                &mut eng,
                &repo,
                &viewer,
                sync_wiki,
                wiki_root.as_deref(),
                low_coverage_threshold,
                write_page,
                &schema,
            )?;
        }
        Cmd::Fix {
            dry_run,
            auto_only,
            write,
        } => {
            run_fix_job(
                &mut eng,
                &repo,
                &viewer,
                sync_wiki,
                wiki_root.as_deref(),
                &schema,
                dry_run,
                auto_only,
                write,
            )?;
        }
        Cmd::Promote { claim_id } => {
            let cid = wiki_core::ClaimId(uuid::Uuid::parse_str(&claim_id)?);
            eng.promote_if_qualified(cid, "cli", &viewer)?;
            eng.save_to_repo_and_flush_outbox_with_policy(&repo, 128, 3)?;
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
            eng.save_to_repo_and_flush_outbox_with_policy(&repo, 128, 3)?;
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!("promoted page {page_id} to {to_status:?}");
        }
        Cmd::ConsistencyAudit => {
            let wiki_dir = wiki_root
                .clone()
                .ok_or("--wiki-dir is required for consistency-audit")?;
            let (report, files) = consistency::run_consistency_audit(
                &repo,
                consistency::ConsistencyAuditOptions {
                    db_path: cli.db.clone(),
                    wiki_dir,
                    palace_path: cli.palace.clone(),
                    generated_at: OffsetDateTime::now_utc(),
                },
            )?;
            println!(
                "consistency_audit db_pages={} db_sources={} vault_empty_unmanaged={} palace_missing_page_drawers={}",
                report.db.page_count,
                report.db.source_count,
                report.vault.empty_unmanaged_files.len(),
                report.palace.missing_page_drawers.len()
            );
            println!("json_report_file={}", files.json_path.display());
            println!("markdown_report_file={}", files.markdown_path.display());
        }
        Cmd::ConsistencyPlan {
            audit_report,
            report_dir,
        } => {
            let (plan, files) = consistency::run_consistency_plan(
                &audit_report,
                report_dir,
                OffsetDateTime::now_utc(),
            )?;
            println!(
                "consistency_plan actions={} executable_actions={}",
                plan.actions.len(),
                plan.actions
                    .iter()
                    .filter(|action| action.executable)
                    .count()
            );
            println!("json_report_file={}", files.json_path.display());
            println!("markdown_report_file={}", files.markdown_path.display());
        }
        Cmd::ConsistencyApply { plan, apply } => {
            let wiki_dir = wiki_root
                .clone()
                .ok_or("--wiki-dir is required for consistency-apply")?;
            let palace_bank = palace_init::mempalace_bank_from_viewer_scope(&cli.viewer_scope);
            let report = consistency::run_consistency_apply(
                &repo,
                &mut eng.store,
                &eng.audits,
                consistency::ConsistencyApplyOptions {
                    plan_path: &plan,
                    wiki_dir: &wiki_dir,
                    palace_path: cli.palace.as_deref(),
                    palace_bank_id: &palace_bank,
                    apply,
                },
            )?;
            println!(
                "consistency_apply mode={} actions_seen={} executable_actions={} db_fixes_applied={} vault_cleanups_applied={} palace_replays_applied={} projection_ran={}",
                report.mode,
                report.actions_seen,
                report.executable_actions,
                report.db_fixes_applied,
                report.vault_cleanups_applied,
                report.palace_replays_applied,
                report.projection_ran,
            );
        }
        Cmd::Crystallize {
            question,
            findings,
            files,
            lessons,
            entry_type,
        } => {
            let et = parse_entry_type_opt(&entry_type)?.unwrap_or(EntryType::Synthesis);
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
            // 用 finalize 替代手动覆盖
            if let Some(page) = eng.store.pages.get_mut(&draft.page.id) {
                finalize_consumed_page(page, et, Confidence::default(), &schema);
            }
            eng.save_to_repo_and_flush_outbox_with_policy(&repo, 128, 3)?;
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!(
                "page={} claims={}",
                draft.page.id.0,
                draft.claim_candidates.len()
            );
        }
        Cmd::Qa {
            question,
            answer,
            entry_type,
        } => {
            let et = parse_entry_type_opt(&entry_type)?.unwrap_or(EntryType::Qa);
            let status = initial_status_for(Some(&et), &schema);

            let page = PageContract::new(&question, et)
                .with_confidence(Confidence::default())
                .with_source("qa")
                .with_section("问题", &question)
                .with_section("回答", &answer)
                .into_page(viewer.clone(), status);

            let pid = page.id;
            eng.store.pages.insert(pid, page);
            eng.save_to_repo_and_flush_outbox_with_policy(&repo, 128, 3)?;
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!("page={}", pid.0);
        }
        Cmd::Synthesis { topic, body } => {
            let et = EntryType::Synthesis;
            let status = initial_status_for(Some(&et), &schema);

            // body 未提供时从 stdin 读取
            let body_text = match body {
                Some(b) => b,
                None => {
                    let mut buf = String::new();
                    std::io::stdin()
                        .read_to_string(&mut buf)
                        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
                    buf.trim_end().to_string()
                }
            };

            let title = topic.clone();
            let page = PageContract::new(&title, et)
                .with_confidence(Confidence::default())
                .with_source("synthesis")
                .with_section("研究问题", &topic)
                .with_section("综合分析", &body_text)
                .into_page(viewer.clone(), status);

            let pid = page.id;
            eng.store.pages.insert(pid, page);
            eng.save_to_repo_and_flush_outbox_with_policy(&repo, 128, 3)?;
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!("page={}", pid.0);
        }
        Cmd::ExportOutboxNdjson => {
            commands::outbox::run_export_all(&repo)?;
        }
        Cmd::VerifyRowState { .. } => unreachable!(),
        Cmd::ExportOutboxNdjsonFrom {
            consumer_tag,
            last_id,
        } => {
            commands::outbox::run_export_from(&repo, &consumer_tag, last_id)?;
        }
        Cmd::AckOutbox {
            up_to_id,
            consumer_tag,
        } => {
            commands::outbox::run_ack(&repo, up_to_id, &consumer_tag)?;
        }
        Cmd::ConsumeToMempalace {
            last_id,
            consumer_tag,
        } => {
            let (dispatch, start_id, acked) = run_consume_to_mempalace_job(
                &eng,
                &repo,
                &consumer_tag,
                last_id,
                cli.palace.as_deref(),
                &cli.viewer_scope,
            )?;
            println!(
                "seen={} dispatched={} ignored={} filtered={} unresolved={} start_id={start_id} acked={acked} consumer_tag={consumer_tag}",
                dispatch.lines_seen,
                dispatch.dispatched,
                dispatch.ignored,
                dispatch.filtered,
                dispatch.unresolved,
            );
        }
        Cmd::PalaceInit {
            last_id,
            consumer_tag,
            report_dir,
        } => {
            let palace_path = cli
                .palace
                .as_deref()
                .ok_or("--palace is required for palace-init")?;
            let resolver = EngineResolver { store: &eng.store };
            let report = palace_init::run_live_palace_init(
                &repo,
                &resolver,
                palace_path,
                &cli.viewer_scope,
                &consumer_tag,
                last_id,
            )?;
            let report_dir = report_dir
                .map(|path| resolve_wiki_relative_path(wiki_root.as_deref(), path))
                .unwrap_or_else(|| {
                    wiki_root
                        .as_deref()
                        .map(|root| root.join("reports"))
                        .unwrap_or_else(|| PathBuf::from("reports"))
                });
            let files = palace_init::write_report_files(&report_dir, &report)?;
            println!(
                "palace_init seen={} dispatched={} ignored={} filtered={} unresolved={} start_id={} acked={} consumer_tag={} drawers={} kg_facts={}",
                report.dispatch.lines_seen,
                report.dispatch.dispatched,
                report.dispatch.ignored,
                report.dispatch.filtered,
                report.dispatch.unresolved,
                report.start_id,
                report.acked,
                report.consumer_tag,
                report.drawer_count.unwrap_or_default(),
                report.kg_fact_count.unwrap_or_default(),
            );
            if let Some(validation) = &report.validation {
                println!(
                    "validation query_ok={} explain_ok={} fusion_ok={} sample_query={}",
                    validation.query_ok,
                    validation.explain_ok,
                    validation.fusion_ok,
                    validation.sample_query,
                );
            }
            println!("json_report_file={}", files.json_path.display());
            println!("markdown_report_file={}", files.markdown_path.display());
        }
        Cmd::Governance {
            cmd:
                GovernanceCmd::Scan {
                    json,
                    report_dir,
                    low_coverage_threshold,
                },
        } => {
            let now = OffsetDateTime::now_utc();
            let report = run_governance_scan(
                &eng.store,
                &schema,
                GovernanceScanOptions {
                    viewer_scope: Some(&viewer),
                    low_coverage_threshold,
                    generated_at: now,
                    report_id: governance::governance_report_prefix(now),
                },
            );
            let files = report_dir
                .map(|dir| resolve_wiki_relative_path(wiki_root.as_deref(), dir))
                .map(|dir| governance::write_scan_files(&report, &dir))
                .transpose()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", governance::render_scan_text(&report));
                if let Some(files) = files {
                    println!("json_report_file={}", files.json_path.display());
                    println!("markdown_report_file={}", files.markdown_path.display());
                }
            }
        }
        Cmd::ResearchSynthesis {
            cmd:
                ResearchSynthesisCmd::Discover {
                    scan: None,
                    json,
                    report_dir,
                    max_single_double,
                    max_triple,
                    max_quad,
                },
        } => {
            let now = OffsetDateTime::now_utc();
            let scan_report = run_governance_scan(
                &eng.store,
                &schema,
                GovernanceScanOptions {
                    viewer_scope: Some(&viewer),
                    low_coverage_threshold: 2,
                    generated_at: now,
                    report_id: governance::governance_report_prefix(now),
                },
            );
            let report = discover_synthesis_candidates(
                &scan_report,
                SynthesisDiscoveryOptions {
                    generated_at: now,
                    report_id: governance::synthesis_discovery_report_prefix(now),
                    max_single_double,
                    max_triple,
                    max_quad,
                },
            );
            let files = report_dir
                .map(|dir| resolve_wiki_relative_path(wiki_root.as_deref(), dir))
                .map(|dir| governance::write_synthesis_discovery_files(&report, &dir))
                .transpose()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", governance::render_synthesis_discovery_text(&report));
                if let Some(files) = files {
                    println!("json_report_file={}", files.json_path.display());
                    println!("markdown_report_file={}", files.markdown_path.display());
                }
            }
        }
        Cmd::ResearchSynthesis {
            cmd:
                ResearchSynthesisCmd::Compose {
                    candidate,
                    discovery,
                    web_evidence,
                    draft_json,
                    verifier_json,
                    internal_only,
                    allow_private_web_search,
                    apply,
                    json,
                    report_dir,
                },
        } => {
            let discovery_report = match discovery {
                Some(path) => {
                    let path = resolve_wiki_relative_path(wiki_root.as_deref(), path);
                    research_synthesis::read_discovery_report(&path)?
                }
                None => {
                    let now = OffsetDateTime::now_utc();
                    let scan_report = run_governance_scan(
                        &eng.store,
                        &schema,
                        GovernanceScanOptions {
                            viewer_scope: Some(&viewer),
                            low_coverage_threshold: 2,
                            generated_at: now,
                            report_id: governance::governance_report_prefix(now),
                        },
                    );
                    discover_synthesis_candidates(
                        &scan_report,
                        SynthesisDiscoveryOptions {
                            generated_at: now,
                            report_id: governance::synthesis_discovery_report_prefix(now),
                            max_single_double: 1,
                            max_triple: 1,
                            max_quad: 1,
                        },
                    )
                }
            };
            let report = run_research_synthesis_compose(
                &mut eng,
                &repo,
                &viewer,
                sync_wiki,
                wiki_root.as_deref(),
                &cli.llm_config,
                &discovery_report,
                &candidate,
                ResearchSynthesisComposeInputs {
                    web_evidence,
                    draft_json,
                    verifier_json,
                    internal_only,
                    allow_private_web_search,
                    apply,
                },
            )?;
            let files = report_dir
                .map(|dir| resolve_wiki_relative_path(wiki_root.as_deref(), dir))
                .map(|dir| research_synthesis::write_compose_files(&report, &dir))
                .transpose()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", research_synthesis::render_compose_text(&report));
                if let Some(files) = files {
                    println!("json_report_file={}", files.json_path.display());
                    println!("markdown_report_file={}", files.markdown_path.display());
                }
            }
        }
        Cmd::ResearchSynthesis {
            cmd:
                ResearchSynthesisCmd::Run {
                    apply,
                    internal_only,
                    allow_private_web_search,
                    json,
                    report_dir,
                    max_single_double,
                    max_triple,
                    max_quad,
                },
        } => {
            let now = OffsetDateTime::now_utc();
            let scan_report = run_governance_scan(
                &eng.store,
                &schema,
                GovernanceScanOptions {
                    viewer_scope: Some(&viewer),
                    low_coverage_threshold: 2,
                    generated_at: now,
                    report_id: governance::governance_report_prefix(now),
                },
            );
            let discovery_report = discover_synthesis_candidates(
                &scan_report,
                SynthesisDiscoveryOptions {
                    generated_at: now,
                    report_id: governance::synthesis_discovery_report_prefix(now),
                    max_single_double,
                    max_triple,
                    max_quad,
                },
            );
            let mut reports = Vec::new();
            for candidate in &discovery_report.candidates {
                reports.push(run_research_synthesis_compose(
                    &mut eng,
                    &repo,
                    &viewer,
                    sync_wiki,
                    wiki_root.as_deref(),
                    &cli.llm_config,
                    &discovery_report,
                    &candidate.candidate_id,
                    ResearchSynthesisComposeInputs {
                        web_evidence: None,
                        draft_json: None,
                        verifier_json: None,
                        internal_only,
                        allow_private_web_search,
                        apply,
                    },
                )?);
            }
            if let Some(dir) = report_dir {
                let dir = resolve_wiki_relative_path(wiki_root.as_deref(), dir);
                for report in &reports {
                    let files = research_synthesis::write_compose_files(report, &dir)?;
                    println!("json_report_file={}", files.json_path.display());
                    println!("markdown_report_file={}", files.markdown_path.display());
                }
            }
            if json {
                println!("{}", serde_json::to_string_pretty(&reports)?);
            } else {
                println!("synthesis run: reports={}", reports.len());
                for report in &reports {
                    print!("{}", research_synthesis::render_compose_text(report));
                }
            }
        }
        Cmd::Governance {
            cmd:
                GovernanceCmd::FixerApply {
                    plan,
                    policy,
                    apply,
                    json,
                    report_dir,
                },
        } => {
            let plan_path = resolve_wiki_relative_path(wiki_root.as_deref(), plan);
            let plan: wiki_core::EvidenceFixerPlan =
                serde_json::from_str(&std::fs::read_to_string(&plan_path)?)?;
            let now = OffsetDateTime::now_utc();
            let report = apply_evidence_fixer_plan(
                &mut eng,
                &viewer,
                &plan,
                EvidenceFixerApplyOptions {
                    generated_at: now,
                    report_id: governance::fixer_apply_report_prefix(now),
                    policy: policy.into(),
                    apply,
                },
            );
            if apply && report.summary.applied > 0 {
                eng.save_to_repo_and_flush_outbox_with_policy(&repo, 128, 3)?;
                maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            }
            let files = report_dir
                .map(|dir| resolve_wiki_relative_path(wiki_root.as_deref(), dir))
                .map(|dir| governance::write_fixer_apply_files(&report, &dir))
                .transpose()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", governance::render_fixer_apply_text(&report));
                if let Some(files) = files {
                    println!("json_report_file={}", files.json_path.display());
                    println!("markdown_report_file={}", files.markdown_path.display());
                    for path in files.tombstone_paths {
                        println!("tombstone_file={}", path.display());
                    }
                }
            }
        }
        Cmd::Governance {
            cmd:
                GovernanceCmd::Restore {
                    tombstone,
                    apply,
                    json,
                    report_dir,
                },
        } => {
            let tombstone_path = resolve_wiki_relative_path(wiki_root.as_deref(), tombstone);
            let tombstone: wiki_core::EvidenceFixerTombstone =
                serde_json::from_str(&std::fs::read_to_string(&tombstone_path)?)?;
            let now = OffsetDateTime::now_utc();
            let report = restore_evidence_fixer_tombstone(
                &mut eng,
                &viewer,
                &tombstone,
                apply,
                now,
                governance::fixer_restore_report_prefix(now),
            );
            if apply && report.status == wiki_core::EvidenceFixerApplyActionStatus::Applied {
                eng.save_to_repo_and_flush_outbox_with_policy(&repo, 128, 3)?;
                maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            }
            let files = report_dir
                .map(|dir| resolve_wiki_relative_path(wiki_root.as_deref(), dir))
                .map(|dir| governance::write_fixer_restore_files(&report, &dir))
                .transpose()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", governance::render_fixer_restore_text(&report));
                if let Some(files) = files {
                    println!("json_report_file={}", files.json_path.display());
                    println!("markdown_report_file={}", files.markdown_path.display());
                }
            }
        }
        Cmd::Metrics {
            consumer_tag,
            low_coverage_threshold,
            json,
            report,
        } => {
            let outbox_stats = repo.get_outbox_stats()?;
            let outbox_progress = repo.get_outbox_consumer_progress(&consumer_tag)?;
            let metrics = collect_wiki_metrics(
                &eng.store,
                &schema,
                Some(&viewer),
                Some(&outbox_stats),
                Some(&outbox_progress),
                low_coverage_threshold,
                OffsetDateTime::now_utc(),
            );
            if let Some(path) = report {
                let path = resolve_wiki_relative_path(wiki_root.as_deref(), path);
                ensure_parent_dir(&path)?;
                std::fs::write(&path, render_metrics_markdown(&metrics))?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&metrics)?);
                } else {
                    print!("{}", render_metrics_text(&metrics));
                    println!("report_file={}", path.display());
                }
            } else if json {
                println!("{}", serde_json::to_string_pretty(&metrics)?);
            } else {
                print!("{}", render_metrics_text(&metrics));
            }
        }
        Cmd::Dashboard {
            output,
            consumer_tag,
            low_coverage_threshold,
        } => {
            let outbox_stats = repo.get_outbox_stats()?;
            let outbox_progress = repo.get_outbox_consumer_progress(&consumer_tag)?;
            let now = OffsetDateTime::now_utc();
            let metrics = collect_wiki_metrics(
                &eng.store,
                &schema,
                Some(&viewer),
                Some(&outbox_stats),
                Some(&outbox_progress),
                low_coverage_threshold,
                now,
            );
            let health = collect_automation_health_report(
                &repo,
                &automation_all_jobs(),
                &consumer_tag,
                now,
            )?;
            let html = dashboard::render_dashboard_html(&health, &metrics, &consumer_tag);
            let output = output
                .map(|path| resolve_wiki_relative_path(wiki_root.as_deref(), path))
                .unwrap_or_else(|| default_dashboard_output(wiki_root.as_deref()));
            ensure_parent_dir(&output)?;
            std::fs::write(&output, html)?;
            println!("dashboard_file={}", output.display());
        }
        Cmd::Suggest {
            consumer_tag,
            low_coverage_threshold,
            json,
            executor_plan,
            report_dir,
        } => {
            let now = OffsetDateTime::now_utc();
            let outbox_stats = repo.get_outbox_stats()?;
            let outbox_progress = repo.get_outbox_consumer_progress(&consumer_tag)?;
            let metrics = collect_wiki_metrics(
                &eng.store,
                &schema,
                Some(&viewer),
                Some(&outbox_stats),
                Some(&outbox_progress),
                low_coverage_threshold,
                now,
            );
            let query_events = parse_outbox_events(&repo.export_outbox_ndjson()?)?;
            let report_id = strategy_report_prefix(now);
            let report = run_strategy_scan(
                &eng.store,
                &schema,
                &metrics,
                &query_events,
                StrategyScanOptions {
                    viewer_scope: Some(&viewer),
                    low_coverage_threshold,
                    generated_at: now,
                    report_id,
                },
            );
            let plan = executor_plan.then(|| build_strategy_execution_plan(&report));

            let report_dir = match report_dir {
                Some(Some(dir)) => Some(resolve_wiki_relative_path(wiki_root.as_deref(), dir)),
                Some(None) => Some(default_suggest_report_dir(wiki_root.as_deref())),
                None => None,
            };

            if let Some(dir) = report_dir {
                std::fs::create_dir_all(&dir)?;
                let json_name = format!("{}.json", report.report_id);
                let markdown_name = format!("{}.md", report.report_id);
                let json_path = dir.join(&json_name);
                let markdown_path = dir.join(&markdown_name);
                std::fs::write(&json_path, serde_json::to_string_pretty(&report)?)?;
                std::fs::write(
                    &markdown_path,
                    render_strategy_report_markdown(&report, &json_name),
                )?;
                let plan_paths = if let Some(plan) = &plan {
                    let plan_json_name = format!("{}.json", plan.plan_id);
                    let plan_markdown_name = format!("{}.md", plan.plan_id);
                    let plan_json_path = dir.join(&plan_json_name);
                    let plan_markdown_path = dir.join(&plan_markdown_name);
                    std::fs::write(&plan_json_path, serde_json::to_string_pretty(plan)?)?;
                    std::fs::write(
                        &plan_markdown_path,
                        render_strategy_execution_plan_markdown(plan, &plan_json_name, &json_name),
                    )?;
                    Some((plan_json_path, plan_markdown_path))
                } else {
                    None
                };
                if json {
                    println!(
                        "{}",
                        serialize_strategy_suggest_json(&report, plan.as_ref())?
                    );
                } else {
                    print!("{}", render_strategy_report_text(&report));
                    if let Some(plan) = &plan {
                        print!("{}", render_strategy_execution_plan_text(plan));
                    }
                    println!("json_report_file={}", json_path.display());
                    println!("markdown_report_file={}", markdown_path.display());
                    if let Some((plan_json_path, plan_markdown_path)) = plan_paths {
                        println!("executor_plan_json_file={}", plan_json_path.display());
                        println!(
                            "executor_plan_markdown_file={}",
                            plan_markdown_path.display()
                        );
                    }
                }
            } else if json {
                println!(
                    "{}",
                    serialize_strategy_suggest_json(&report, plan.as_ref())?
                );
            } else {
                print!("{}", render_strategy_report_text(&report));
                if let Some(plan) = &plan {
                    print!("{}", render_strategy_execution_plan_text(plan));
                }
            }
        }
        Cmd::SuggestExecutorApply {
            plan,
            allow,
            apply,
            json,
            report_dir,
        } => {
            let plan_text = std::fs::read_to_string(&plan)?;
            let plan_data: StrategyExecutionPlan = serde_json::from_str(&plan_text)
                .map_err(|err| format!("executor plan JSON parse error: {err}"))?;
            let report_dir = match report_dir {
                Some(Some(dir)) => Some(resolve_wiki_relative_path(wiki_root.as_deref(), dir)),
                Some(None) => Some(default_suggest_report_dir(wiki_root.as_deref())),
                None => None,
            };
            if let Some(dir) = &report_dir {
                std::fs::create_dir_all(dir)?;
                let probe_path = dir.join(".m12-executor-write-check");
                std::fs::write(&probe_path, b"")?;
                let _ = std::fs::remove_file(&probe_path);
            }
            let report = build_strategy_executor_apply_report(
                &mut eng,
                &repo,
                &viewer,
                sync_wiki,
                wiki_root.as_deref(),
                &plan_data,
                &allow,
                apply,
            )?;
            let report_paths = if let Some(dir) = report_dir {
                let json_name = format!("{}.json", report.report_id);
                let markdown_name = format!("{}.md", report.report_id);
                let json_path = dir.join(&json_name);
                let markdown_path = dir.join(&markdown_name);
                let plan_json_name = plan
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("executor-plan.json");
                std::fs::write(&json_path, serde_json::to_string_pretty(&report)?)?;
                std::fs::write(
                    &markdown_path,
                    render_strategy_executor_apply_report_markdown(
                        &report,
                        &json_name,
                        plan_json_name,
                    ),
                )?;
                Some((json_path, markdown_path))
            } else {
                None
            };

            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", render_strategy_executor_apply_report_text(&report));
                if let Some((json_path, markdown_path)) = report_paths {
                    println!("executor_apply_json_file={}", json_path.display());
                    println!("executor_apply_markdown_file={}", markdown_path.display());
                }
            }
        }
        Cmd::LlmSmoke { config, prompt } => {
            commands::llm_smoke::run(config, prompt)?;
        }
        Cmd::Mcp { once } => {
            mcp::run_mcp(
                &cli.db,
                schema,
                &cli.viewer_scope,
                once,
                &cli.llm_config,
                cli.vectors,
                if sync_wiki {
                    wiki_root.as_deref()
                } else {
                    None
                },
                cli.palace
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .as_deref(),
            )?;
        }
        Cmd::Maintenance => {
            run_maintenance_job(&mut eng, &repo, &viewer, sync_wiki, wiki_root.as_deref())?;
        }
        Cmd::BatchIngest {
            ref vault,
            ref origin,
            ref source_path,
            ref scope,
            limit,
            dry_run,
            delay_secs,
        } => {
            let vault_dir = vault
                .clone()
                .unwrap_or_else(wiki_compiler::default_vault_path);
            let heartbeat = AutomationHeartbeat {
                repo: &repo,
                run_id: None,
            };
            wiki_compiler::batch_ingest_cmd(
                &mut eng,
                &repo,
                &cli.llm_config,
                cli.vectors,
                &schema,
                &heartbeat,
                wiki_compiler::BatchIngestOptions {
                    vault: &vault_dir,
                    limit,
                    dry_run,
                    delay_secs,
                    sync_wiki,
                    wiki_root: wiki_root.as_deref(),
                    scope: scope.as_deref(),
                    origin: origin.as_deref(),
                    source_path: source_path.as_deref(),
                },
            )?;
        }
        Cmd::CompilerResolveDeferred {
            ref report,
            apply,
            allow_create,
            ref report_dir,
            ref consumer_tag,
        } => {
            let resolved_report_dir = report_dir
                .as_ref()
                .map(|path| resolve_wiki_relative_path(wiki_root.as_deref(), path.clone()))
                .unwrap_or_else(|| {
                    wiki_root
                        .as_deref()
                        .map(|root| root.join("reports"))
                        .unwrap_or_else(|| PathBuf::from("reports"))
                });
            if apply && wiki_root.is_none() {
                return Err("--wiki-dir is required with compiler-resolve-deferred --apply".into());
            }
            if apply && cli.palace.is_none() {
                return Err("--palace is required with compiler-resolve-deferred --apply".into());
            }
            let run = compiler_deferred::run_compiler_deferred_resolution(
                &mut eng,
                &repo,
                compiler_deferred::CompilerDeferredResolutionOptions {
                    report_path: report,
                    report_dir: &resolved_report_dir,
                    apply,
                    allow_create,
                    scope: &viewer,
                    schema: &schema,
                },
            )?;
            if apply && run.db_changed {
                let root = wiki_root
                    .as_deref()
                    .ok_or("--wiki-dir is required with compiler-resolve-deferred --apply")?;
                let projection = write_projection(root, &eng.store, &eng.audits)?;
                println!(
                    "projection pages={} claims={} sources={}",
                    projection.pages_written, projection.claims_written, projection.sources_written
                );
                let (dispatch, start_id, acked) = run_consume_to_mempalace_job(
                    &eng,
                    &repo,
                    consumer_tag,
                    0,
                    cli.palace.as_deref(),
                    &cli.viewer_scope,
                )?;
                println!(
                    "mempalace seen={} dispatched={} ignored={} filtered={} unresolved={} start_id={start_id} acked={acked} consumer_tag={consumer_tag}",
                    dispatch.lines_seen,
                    dispatch.dispatched,
                    dispatch.ignored,
                    dispatch.filtered,
                    dispatch.unresolved,
                );
                run_lint_job(&mut eng, &repo, &viewer, true, wiki_root.as_deref())?;
            }
            println!(
                "compiler_deferred_resolution mode={} aliases_applied={} pages_created={} changed_pages={} json_report={} markdown_report={}",
                if apply { "apply" } else { "dry-run" },
                run.aliases_applied,
                run.pages_created,
                run.changed_page_ids.len(),
                run.json_report_path.display(),
                run.markdown_report_path.display()
            );
        }
        Cmd::Automation {
            cmd: AutomationCmd::RunDaily { dry_run: false },
        } => {
            run_daily_automation(
                &cli,
                &mut eng,
                &repo,
                &viewer,
                sync_wiki,
                wiki_root.as_deref(),
                &schema,
            )?;
        }
        Cmd::Automation {
            cmd: AutomationCmd::RunDaily { dry_run: true },
        } => {
            let jobs = automation_run_daily_jobs();
            let mut stdout = std::io::stdout().lock();
            run_automation_plan(&jobs, true, &mut stdout, |_| Ok(()))?;
        }
        Cmd::Automation {
            cmd: AutomationCmd::Run { job },
        } => {
            let mut stdout = std::io::stdout().lock();
            run_single_automation_job(
                &mut stdout,
                job,
                &cli,
                &mut eng,
                &repo,
                &viewer,
                sync_wiki,
                wiki_root.as_deref(),
                &schema,
            )?;
        }
        Cmd::Automation {
            cmd: AutomationCmd::LastFailures { limit },
        } => {
            let mut stdout = std::io::stdout().lock();
            print_automation_last_failures(&repo, limit, &mut stdout)?;
        }
        Cmd::Automation {
            cmd: AutomationCmd::Status,
        } => {
            let jobs = automation_all_jobs();
            let mut stdout = std::io::stdout().lock();
            print_automation_status(&repo, &jobs, &mut stdout)?;
        }
        Cmd::Automation {
            cmd: AutomationCmd::Doctor { consumer_tag },
        } => {
            let jobs = automation_all_jobs();
            let mut stdout = std::io::stdout().lock();
            print_automation_doctor(&repo, &jobs, &consumer_tag, &mut stdout)?;
        }
        Cmd::Automation {
            cmd:
                AutomationCmd::Health {
                    consumer_tag,
                    summary_file,
                    exit_on_yellow,
                },
        } => {
            let report = collect_automation_health_report(
                &repo,
                &automation_all_jobs(),
                &consumer_tag,
                OffsetDateTime::now_utc(),
            )?;
            let rendered = render_automation_health_report(&report, &consumer_tag);
            print!("{rendered}");
            if let Some(path) = summary_file {
                let path = resolve_wiki_relative_path(wiki_root.as_deref(), path);
                ensure_parent_dir(&path)?;
                std::fs::write(&path, &rendered)?;
                println!("summary_file={}", path.display());
            }
            emit_automation_health_alert(report.level);
            let should_exit = match report.level {
                AutomationHealthLevel::Red => true,
                AutomationHealthLevel::Yellow => exit_on_yellow,
                AutomationHealthLevel::Green => false,
            };
            if should_exit {
                std::process::exit(1);
            }
        }
        Cmd::Automation {
            cmd: AutomationCmd::VerifyRestore,
        } => {
            let wiki_root = wiki_root
                .as_deref()
                .ok_or_else(|| "--wiki-dir 是 automation verify-restore 的必填参数".to_string())?;
            let report = collect_restore_verify_report(
                &cli.db,
                &repo,
                wiki_root,
                cli.palace.as_deref(),
                DEFAULT_MEMPALACE_CONSUMER_TAG,
            )?;
            print!(
                "{}",
                render_restore_verify_report(&report, DEFAULT_MEMPALACE_CONSUMER_TAG)
            );
        }
        Cmd::Automation {
            cmd: AutomationCmd::ListJobs,
        } => unreachable!(),
        Cmd::VaultAudit { .. } | Cmd::OrphanGovernance { .. } | Cmd::VaultBackfill { .. } => {
            unreachable!()
        }
        // SchemaValidate 已在 main() 中短路，此处不可达
        Cmd::SchemaValidate { .. } => unreachable!(),
        Cmd::NotionSync {
            db_id,
            since,
            limit,
            dry_run,
            request_delay_ms,
            writeback_notion,
            refresh_existing,
            tag_policy,
            verbose,
        } => {
            apply_notion_sync_tag_policy(&mut eng.schema, tag_policy);
            run_notion_sync_cmd(
                &mut eng,
                &repo,
                &viewer,
                if cli.sync_wiki {
                    wiki_root.as_deref()
                } else {
                    None
                },
                db_id,
                since.as_deref(),
                limit,
                dry_run,
                request_delay_ms,
                writeback_notion,
                refresh_existing,
                verbose,
            )?;
        }
        Cmd::NotionSyncIndexBackfill {
            vault,
            dry_run,
            apply,
        } => {
            if dry_run && apply {
                return Err("--dry-run and --apply are mutually exclusive".into());
            }
            let vault = vault
                .clone()
                .or_else(|| wiki_root.clone())
                .ok_or("--vault or --wiki-dir is required for notion-sync-index-backfill")?;
            let mode = if apply {
                notion_index_backfill::BackfillMode::Apply
            } else {
                notion_index_backfill::BackfillMode::DryRun
            };
            let report = notion_index_backfill::backfill_notion_page_index(&repo, &vault, mode)?;
            println!("{report}");
        }
        Cmd::NotionSourceVaultSync {
            vault,
            dry_run,
            apply,
            repair_tags,
            refresh_existing,
        } => {
            if dry_run && apply {
                return Err("--dry-run and --apply are mutually exclusive".into());
            }
            let vault = vault
                .clone()
                .or_else(|| wiki_root.clone())
                .ok_or("--vault or --wiki-dir is required for notion-source-vault-sync")?;
            let mode = if apply {
                notion_source_projection::ProjectionMode::Apply
            } else {
                notion_source_projection::ProjectionMode::DryRun
            };
            let sources: Vec<_> = eng.store.sources.values().cloned().collect();
            let report = if refresh_existing {
                notion_source_projection::project_notion_sources_to_vault_with_options(
                    &sources,
                    &vault,
                    notion_source_projection::ProjectionOptions {
                        mode,
                        refresh_existing,
                    },
                )?
            } else {
                notion_source_projection::project_notion_sources_to_vault(&sources, &vault, mode)?
            };
            println!("{report}");
            if repair_tags {
                let report = notion_source_projection::repair_obsidian_source_tags(&vault, mode)?;
                println!("{report}");
            }
        }
        Cmd::NotionArchivedRetirement { command } => match command {
            NotionArchivedRetirementCmd::Plan {
                report_dir,
                limit,
                request_delay_ms,
            } => {
                let mut indexes = repo.list_notion_page_indexes()?;
                if let Some(limit) = limit {
                    indexes.truncate(limit);
                }

                let mut client =
                    notion_client::NotionApiClient::from_env_with_delay(request_delay_ms)?;
                let mut states = BTreeMap::new();
                let mut fetch_errors = Vec::new();
                for record in &indexes {
                    match client.retrieve_page_archive_state(&record.notion_page_id) {
                        Ok(state) => {
                            states.insert(canonical_notion_page_id(&state.id), state);
                        }
                        Err(err) => fetch_errors.push(
                            notion_archived_retirement::NotionArchivedRetirementFetchError {
                                db_id: record.db_id.clone(),
                                notion_page_id: record.notion_page_id.clone(),
                                source_id: record.source_id.0.to_string(),
                                error: err.to_string(),
                            },
                        ),
                    }
                }

                let sources: Vec<_> = eng.store.sources.values().cloned().collect();
                let report = notion_archived_retirement::build_notion_archived_retirement_report(
                    &indexes,
                    &sources,
                    &states,
                    fetch_errors,
                    OffsetDateTime::now_utc(),
                );
                let report_dir = report_dir
                    .map(|path| resolve_wiki_relative_path(wiki_root.as_deref(), path))
                    .unwrap_or_else(|| {
                        wiki_root
                            .as_deref()
                            .map(|root| root.join("reports"))
                            .unwrap_or_else(|| PathBuf::from("reports"))
                    });
                let files = notion_archived_retirement::write_notion_archived_retirement_report(
                    &report,
                    &report_dir,
                )?;
                println!(
                    "notion_archived_retirement_plan indexed={} archived_candidates={} active_pages={} missing_sources={} fetch_errors={}",
                    report.total_indexed_pages,
                    report.archived_candidates,
                    report.active_pages,
                    report.missing_sources,
                    report.fetch_errors.len()
                );
                println!("json_report_file={}", files.json_path.display());
                println!("markdown_report_file={}", files.markdown_path.display());
            }
            NotionArchivedRetirementCmd::Apply {
                plan,
                apply,
                report_dir,
            } => {
                if apply && wiki_root.is_none() {
                    return Err(
                        "--wiki-dir is required with notion-archived-retirement apply --apply"
                            .into(),
                    );
                }
                let plan_path = resolve_wiki_relative_path(wiki_root.as_deref(), plan);
                let plan_report =
                    notion_archived_retirement::read_notion_archived_retirement_report(&plan_path)?;
                if plan_report.version != 1 || plan_report.mode != "dry_run" {
                    return Err(format!(
                        "unsupported notion archived retirement plan: version={} mode={}",
                        plan_report.version, plan_report.mode
                    )
                    .into());
                }
                let sources: Vec<_> = eng.store.sources.values().cloned().collect();
                let mut apply_plan =
                    notion_archived_retirement::build_notion_archived_retirement_apply_plan(
                        &plan_report,
                        &sources,
                        apply,
                        OffsetDateTime::now_utc(),
                    );
                let vault_files = if let Some(root) = wiki_root.as_deref() {
                    notion_archived_retirement::collect_retired_source_files(
                        root,
                        &apply_plan.source_ids,
                        &apply_plan.notion_page_ids,
                    )?
                } else {
                    Vec::new()
                };
                apply_plan.report.vault_files_planned = vault_files.len();
                apply_plan.report.vault_files = vault_files
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect();

                if apply {
                    for source_id in &apply_plan.source_ids {
                        eng.store.sources.remove(source_id);
                        eng.audits.push(AuditRecord::new(
                            AuditOperation::RetireSource,
                            "notion-archived-retirement",
                            format!("retired notion source {}", source_id.0),
                        ));
                    }
                    let snapshot = eng.store.to_snapshot(&eng.audits);
                    let deleted_index_rows = repo.save_snapshot_and_delete_notion_page_indexes(
                        &snapshot,
                        &apply_plan.notion_page_ids,
                    )?;
                    let deleted_vault_files =
                        notion_archived_retirement::delete_retired_source_files(&vault_files)?;
                    apply_plan.report.sources_removed = apply_plan.source_ids.len();
                    apply_plan.report.index_rows_deleted = deleted_index_rows;
                    apply_plan.report.vault_files_deleted = deleted_vault_files;
                    apply_plan.report.applied_source_ids = apply_plan
                        .source_ids
                        .iter()
                        .map(|source_id| source_id.0.to_string())
                        .collect();
                    apply_plan.report.applied_notion_page_ids = apply_plan.notion_page_ids.clone();
                }

                let report_dir = report_dir
                    .map(|path| resolve_wiki_relative_path(wiki_root.as_deref(), path))
                    .unwrap_or_else(|| {
                        wiki_root
                            .as_deref()
                            .map(|root| root.join("reports"))
                            .unwrap_or_else(|| PathBuf::from("reports"))
                    });
                let files =
                    notion_archived_retirement::write_notion_archived_retirement_apply_report(
                        &apply_plan.report,
                        &report_dir,
                    )?;
                println!(
                    "notion_archived_retirement_apply mode={} actions_seen={} safe_candidates={} unsafe_candidates={} stale_candidates={} sources_planned={} sources_removed={} index_rows_deleted={} vault_files_planned={} vault_files_deleted={}",
                    apply_plan.report.mode,
                    apply_plan.report.actions_seen,
                    apply_plan.report.safe_candidates,
                    apply_plan.report.unsafe_candidates,
                    apply_plan.report.stale_candidates,
                    apply_plan.report.sources_planned,
                    apply_plan.report.sources_removed,
                    apply_plan.report.index_rows_deleted,
                    apply_plan.report.vault_files_planned,
                    apply_plan.report.vault_files_deleted,
                );
                println!("json_report_file={}", files.json_path.display());
                println!("markdown_report_file={}", files.markdown_path.display());
            }
        },
    }
    Ok(())
}

use commands::query::{
    build_wiki_search_ports, doc_id_visible_to_viewer, filter_graph_extras_for_viewer,
    merge_optional_graph_extras, run_fusion_query,
};

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;
    use wiki_core::DomainSchema;
    use wiki_storage::{
        AutomationJobFailureSummary, AutomationRunRecord, AutomationRunStatus,
        OutboxConsumerProgress, OutboxStats,
    };

    fn cmd_needs_writer_lease(cmd: &Cmd) -> bool {
        cmd.needs_writer_lease()
    }

    fn sample_record(
        status: AutomationRunStatus,
        heartbeat_at: OffsetDateTime,
    ) -> AutomationRunRecord {
        AutomationRunRecord {
            id: 1,
            job_name: "lint".into(),
            started_at: heartbeat_at - Duration::minutes(5),
            finished_at: None,
            status,
            duration_ms: None,
            error_summary: None,
            heartbeat_at,
        }
    }

    fn test_automation_health_thresholds() -> AutomationHealthThresholds {
        AutomationHealthThresholds {
            stale_heartbeat_yellow: Duration::hours(6),
            stale_heartbeat_red: Duration::hours(24),
            consecutive_failures_yellow: 2,
            consecutive_failures_red: 3,
            backlog_yellow: 25,
            backlog_red: 100,
        }
    }

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

    #[test]
    fn tag_cli_ingest_accepts_repeatable_tag_args() {
        let cli = Cli::try_parse_from([
            "wiki",
            "ingest",
            "file:///a.md",
            "body",
            "--tag",
            "alpha",
            "--tag",
            "beta",
        ])
        .expect("CLI args should parse");

        match cli.cmd {
            Cmd::Ingest { tags, .. } => {
                assert_eq!(tags, vec!["alpha".to_string(), "beta".to_string()]);
            }
            _ => panic!("expected ingest command"),
        }
    }

    #[test]
    fn tag_cli_file_claim_accepts_repeatable_tag_args() {
        let cli = Cli::try_parse_from([
            "wiki",
            "file-claim",
            "claim text",
            "--tag",
            "alpha",
            "--tag",
            "beta",
        ])
        .expect("CLI args should parse");

        match cli.cmd {
            Cmd::FileClaim { tags, .. } => {
                assert_eq!(tags, vec!["alpha".to_string(), "beta".to_string()]);
            }
            _ => panic!("expected file-claim command"),
        }
    }

    #[test]
    fn tag_batch_context_carries_source_tags_for_ingest() {
        let batch = BatchIngestContext {
            source_title: "source".to_string(),
            source_url: "file:///source.md".to_string(),
            source_tags: vec!["seed".to_string(), "new".to_string()],
        };

        assert_eq!(
            batch_source_tags_for_ingest(&batch),
            &["seed".to_string(), "new".to_string()]
        );
    }

    fn tag_test_plan_with_claim_tags(claim_tags: Vec<String>) -> LlmIngestPlanV1 {
        LlmIngestPlanV1 {
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
                tags: claim_tags,
            }],
            concepts: Vec::new(),
            entities: Vec::new(),
            relationships: Vec::new(),
        }
    }

    #[test]
    fn tag_preflight_counts_source_and_claim_new_tags_per_ingest() {
        let mut schema = DomainSchema::permissive_default();
        schema.tag_config.max_new_tags_per_ingest = 1;
        let source_tags = vec!["new-source".to_string()];
        let plan = tag_test_plan_with_claim_tags(vec!["new-claim".to_string()]);

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

    #[test]
    fn tag_preflight_ignores_seed_and_case_duplicates_for_new_count() {
        let mut schema = DomainSchema::permissive_default();
        schema.tag_config.seed_tags = vec!["Known".into()];
        schema.tag_config.max_new_tags_per_ingest = 1;
        let source_tags = vec!["KNOWN".to_string(), "new".to_string()];
        let plan = tag_test_plan_with_claim_tags(vec!["known".to_string(), "New".to_string()]);

        preflight_llm_plan_tags(&plan, &source_tags, &schema).unwrap();
    }

    #[test]
    fn notion_sync_strict_tag_policy_leaves_schema_unchanged() {
        let mut schema = DomainSchema::permissive_default();
        schema.tag_config.max_new_tags_per_ingest = 1;

        apply_notion_sync_tag_policy(&mut schema, NotionSyncTagPolicy::Strict);

        assert_eq!(schema.tag_config.max_new_tags_per_ingest, 1);
    }

    #[test]
    fn notion_sync_trusted_source_tag_policy_allows_source_tags() {
        let mut schema = DomainSchema::permissive_default();
        schema.tag_config.max_new_tags_per_ingest = 1;
        schema.tag_config.deprecated_tags = vec!["old".into()];

        apply_notion_sync_tag_policy(&mut schema, NotionSyncTagPolicy::TrustedSource);

        assert_eq!(schema.tag_config.max_new_tags_per_ingest, u32::MAX);
        assert!(schema.tag_config.deprecated_tags.is_empty());
    }

    #[test]
    fn graph_extras_filter_private_doc_and_reject_mempalace_ids() {
        let mut store = InMemoryStore::default();
        let viewer = Scope::Private {
            agent_id: "agent1".into(),
        };
        let other = Scope::Private {
            agent_id: "agent2".into(),
        };
        let own_claim = wiki_core::Claim::new("visible", viewer.clone(), MemoryTier::Semantic);
        let other_claim = wiki_core::Claim::new("hidden", other, MemoryTier::Semantic);
        let own_id = format_claim_doc_id(own_claim.id);
        let other_id = format_claim_doc_id(other_claim.id);
        store.claims.insert(own_claim.id, own_claim);
        store.claims.insert(other_claim.id, other_claim);

        let filtered = filter_graph_extras_for_viewer(
            vec![
                other_id,
                "mp_drawer:42".into(),
                "mp_kg:subject:predicate".into(),
                own_id.clone(),
                "weird:thing".into(),
                "claim:not-a-uuid".into(),
            ],
            &store,
            &viewer,
        );

        assert_eq!(filtered, vec![own_id]);
    }

    struct FixedGraphPorts;

    impl SearchPorts for FixedGraphPorts {
        fn bm25_ranked_ids(&self, _query: &str, _limit: usize) -> Vec<String> {
            Vec::new()
        }

        fn vector_ranked_ids(&self, _query: &str, _limit: usize) -> Vec<String> {
            Vec::new()
        }

        fn graph_ranked_ids(&self, _query: &str, _limit: usize) -> Vec<String> {
            vec!["mp_kg:banked".into(), "entity:wiki".into()]
        }
    }

    #[test]
    fn graph_extras_merge_with_active_graph_stream() {
        let merged = merge_optional_graph_extras(
            SearchPorts::graph_ranked_ids(&FixedGraphPorts, "q", 10),
            Some(vec!["entity:extra".into()]),
            10,
        );

        assert_eq!(
            merged,
            vec![
                "mp_kg:banked".to_string(),
                "entity:extra".to_string(),
                "entity:wiki".to_string(),
            ]
        );
    }

    #[test]
    fn automation_run_daily_plan_is_fixed_and_ordered() {
        let jobs = automation_run_daily_jobs();
        let labels: Vec<&str> = jobs.iter().copied().map(automation_job_name).collect();
        assert_eq!(
            labels,
            vec![
                "notion-sync",
                "batch-ingest",
                "governance-scan",
                "fixer-plan",
                "fixer-apply",
                "maintenance",
                "consume-to-mempalace",
                "vault-reports",
            ]
        );
    }

    #[test]
    fn automation_job_registry_lists_named_jobs_in_stable_order() {
        let labels: Vec<&str> = automation_job_specs()
            .iter()
            .map(|spec| automation_job_name(spec.job))
            .collect();
        assert_eq!(
            labels,
            vec![
                "notion-sync",
                "batch-ingest",
                "governance-scan",
                "fixer-plan",
                "fixer-apply",
                "lint",
                "maintenance",
                "consume-to-mempalace",
                "llm-smoke",
                "vault-reports",
                "synthesis-discover",
                "synthesis-run",
            ]
        );
        assert!(automation_job_spec(AutomationJob::FixerPlan).requires_network);
        assert!(automation_job_spec(AutomationJob::FixerPlan).in_daily);
        assert!(!automation_job_spec(AutomationJob::FixerApply).requires_network);
        assert!(automation_job_spec(AutomationJob::FixerApply).in_daily);
        assert!(automation_job_spec(AutomationJob::LlmSmoke).requires_network);
        assert!(!automation_job_spec(AutomationJob::LlmSmoke).in_daily);
        assert!(automation_job_spec(AutomationJob::NotionSync).requires_network);
        assert!(automation_job_spec(AutomationJob::NotionSync).in_daily);
        assert!(!automation_job_spec(AutomationJob::VaultReports).requires_network);
        assert!(automation_job_spec(AutomationJob::VaultReports).in_daily);
        assert!(automation_job_spec(AutomationJob::SynthesisRun).requires_network);
        assert!(!automation_job_spec(AutomationJob::SynthesisRun).in_daily);
        assert!(automation_notion_refresh_existing());
    }

    #[test]
    fn writer_lease_command_classifier_covers_write_and_read_paths() {
        assert!(cmd_needs_writer_lease(&Cmd::Ingest {
            uri: "file:///a.md".into(),
            body: "body".into(),
            scope: "private:cli".into(),
            tags: Vec::new(),
        }));
        assert!(cmd_needs_writer_lease(&Cmd::Query {
            query: "q".into(),
            rrf_k: 60.0,
            per_stream_limit: 50,
            write_page: false,
            page_title: None,
            entry_type: None,
            palace_db: None,
            palace_bank: "wiki".into(),
        }));
        assert!(Cmd::Lint.needs_writer_lease());
        assert!(cmd_needs_writer_lease(&Cmd::Fix {
            dry_run: false,
            auto_only: false,
            write: true,
        }));
        assert!(!cmd_needs_writer_lease(&Cmd::Fix {
            dry_run: true,
            auto_only: false,
            write: true,
        }));
        assert!(cmd_needs_writer_lease(&Cmd::BatchIngest {
            vault: None,
            origin: None,
            source_path: None,
            scope: None,
            limit: None,
            dry_run: false,
            delay_secs: 0,
        }));
        assert!(!cmd_needs_writer_lease(&Cmd::BatchIngest {
            vault: None,
            origin: None,
            source_path: None,
            scope: None,
            limit: None,
            dry_run: true,
            delay_secs: 0,
        }));
        assert!(cmd_needs_writer_lease(&Cmd::Automation {
            cmd: AutomationCmd::RunDaily { dry_run: false },
        }));
        assert!(!cmd_needs_writer_lease(&Cmd::Automation {
            cmd: AutomationCmd::RunDaily { dry_run: true },
        }));
        assert!(cmd_needs_writer_lease(&Cmd::Automation {
            cmd: AutomationCmd::Run {
                job: AutomationJob::Lint,
            },
        }));
        assert!(!cmd_needs_writer_lease(&Cmd::Automation {
            cmd: AutomationCmd::Run {
                job: AutomationJob::LlmSmoke,
            },
        }));
        assert!(cmd_needs_writer_lease(&Cmd::Automation {
            cmd: AutomationCmd::Run {
                job: AutomationJob::FixerApply,
            },
        }));
        assert!(!cmd_needs_writer_lease(&Cmd::Automation {
            cmd: AutomationCmd::Run {
                job: AutomationJob::VaultReports,
            },
        }));
        assert!(!cmd_needs_writer_lease(&Cmd::Automation {
            cmd: AutomationCmd::Run {
                job: AutomationJob::SynthesisDiscover,
            },
        }));
        assert!(cmd_needs_writer_lease(&Cmd::Automation {
            cmd: AutomationCmd::Run {
                job: AutomationJob::SynthesisRun,
            },
        }));
        assert!(cmd_needs_writer_lease(&Cmd::NotionSync {
            db_id: NotionDbTarget::All,
            since: None,
            limit: None,
            dry_run: false,
            request_delay_ms: 350,
            writeback_notion: false,
            refresh_existing: false,
            tag_policy: NotionSyncTagPolicy::TrustedSource,
            verbose: false,
        }));
        assert!(!cmd_needs_writer_lease(&Cmd::NotionSync {
            db_id: NotionDbTarget::All,
            since: None,
            limit: None,
            dry_run: true,
            request_delay_ms: 350,
            writeback_notion: false,
            refresh_existing: false,
            tag_policy: NotionSyncTagPolicy::TrustedSource,
            verbose: false,
        }));
        assert!(!cmd_needs_writer_lease(&Cmd::Metrics {
            consumer_tag: DEFAULT_MEMPALACE_CONSUMER_TAG.into(),
            low_coverage_threshold: 2,
            json: false,
            report: None,
        }));
        assert!(!cmd_needs_writer_lease(&Cmd::Governance {
            cmd: GovernanceCmd::Scan {
                json: false,
                report_dir: None,
                low_coverage_threshold: 2,
            },
        }));
        assert!(!cmd_needs_writer_lease(&Cmd::ResearchSynthesis {
            cmd: ResearchSynthesisCmd::Discover {
                scan: Some(PathBuf::from("scan.json")),
                json: false,
                report_dir: None,
                max_single_double: 1,
                max_triple: 1,
                max_quad: 1,
            },
        }));
        assert!(!cmd_needs_writer_lease(&Cmd::ResearchSynthesis {
            cmd: ResearchSynthesisCmd::Compose {
                candidate: "synth-cand-0001".into(),
                discovery: Some(PathBuf::from("discovery.json")),
                web_evidence: None,
                draft_json: None,
                verifier_json: None,
                internal_only: false,
                allow_private_web_search: false,
                apply: false,
                json: false,
                report_dir: None,
            },
        }));
        assert!(cmd_needs_writer_lease(&Cmd::ResearchSynthesis {
            cmd: ResearchSynthesisCmd::Run {
                apply: true,
                internal_only: false,
                allow_private_web_search: false,
                json: false,
                report_dir: None,
                max_single_double: 1,
                max_triple: 1,
                max_quad: 1,
            },
        }));
        assert!(!cmd_needs_writer_lease(&Cmd::ExportOutboxNdjsonFrom {
            consumer_tag: DEFAULT_MEMPALACE_CONSUMER_TAG.into(),
            last_id: 0,
        }));
        assert!(!cmd_needs_writer_lease(&Cmd::VerifyRowState {
            json: false
        }));
        assert!(!cmd_needs_writer_lease(&Cmd::NotionArchivedRetirement {
            command: NotionArchivedRetirementCmd::Plan {
                report_dir: None,
                limit: None,
                request_delay_ms: 350,
            },
        },));
        assert!(!cmd_needs_writer_lease(&Cmd::NotionArchivedRetirement {
            command: NotionArchivedRetirementCmd::Apply {
                plan: PathBuf::from("plan.json"),
                apply: false,
                report_dir: None,
            },
        },));
        assert!(cmd_needs_writer_lease(&Cmd::NotionArchivedRetirement {
            command: NotionArchivedRetirementCmd::Apply {
                plan: PathBuf::from("plan.json"),
                apply: true,
                report_dir: None,
            },
        },));
        assert!(cmd_needs_writer_lease(&Cmd::NotionSourceVaultSync {
            vault: None,
            dry_run: false,
            apply: true,
            repair_tags: true,
            refresh_existing: true,
        }));
    }

    #[test]
    fn automation_run_daily_dry_run_prints_plan_only() {
        let jobs = automation_run_daily_jobs();
        let mut out = Vec::new();
        let mut called = Vec::new();

        run_automation_plan(&jobs, true, &mut out, |job| {
            called.push(job);
            Ok(())
        })
        .unwrap();

        assert!(called.is_empty());
        let stdout = String::from_utf8(out).unwrap();
        assert!(stdout.contains("automation run-daily plan:"));
        assert!(stdout.contains("1. notion-sync"));
        assert!(stdout.contains("2. batch-ingest"));
        assert!(stdout.contains("3. governance-scan"));
        assert!(stdout.contains("4. fixer-plan"));
        assert!(stdout.contains("5. fixer-apply"));
        assert!(stdout.contains("6. maintenance"));
        assert!(stdout.contains("7. consume-to-mempalace"));
        assert!(stdout.contains("8. vault-reports"));
        assert!(stdout.contains("dry-run: no jobs executed"));
    }

    #[test]
    fn scheduled_report_prune_keeps_newest_runs_and_ignores_latest_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for name in [
            "2026-04-28T000000.000000001Z-scheduled",
            "2026-04-28T000000.000000002Z-scheduled",
            "2026-04-28T000000.000000003Z-scheduled",
        ] {
            std::fs::create_dir_all(root.join(name)).unwrap();
        }
        std::fs::write(root.join("latest.json"), "{}").unwrap();

        let pruned = prune_scheduled_report_runs(root, 2).unwrap();

        assert_eq!(pruned, 1);
        assert!(!root.join("2026-04-28T000000.000000001Z-scheduled").exists());
        assert!(root.join("2026-04-28T000000.000000002Z-scheduled").exists());
        assert!(root.join("2026-04-28T000000.000000003Z-scheduled").exists());
        assert!(root.join("latest.json").exists());
    }

    #[test]
    fn automation_run_daily_stops_after_first_failure() {
        let jobs = automation_run_daily_jobs();
        let mut out = Vec::new();
        let mut seen = Vec::new();

        let err = run_automation_plan(&jobs, false, &mut out, |job| {
            seen.push(job);
            if job == AutomationJob::GovernanceScan {
                Err("boom".into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();

        assert_eq!(
            seen,
            vec![
                AutomationJob::NotionSync,
                AutomationJob::BatchIngest,
                AutomationJob::GovernanceScan,
            ]
        );
        assert!(err.to_string().contains("boom"));
        let stdout = String::from_utf8(out).unwrap();
        assert!(stdout.contains("automation: running notion-sync"));
        assert!(stdout.contains("automation: finished notion-sync"));
        assert!(stdout.contains("automation: running batch-ingest"));
        assert!(stdout.contains("automation: finished batch-ingest"));
        assert!(stdout.contains("automation: running governance-scan"));
        assert!(!stdout.contains("automation: finished governance-scan"));
        assert!(!stdout.contains("automation: running fixer-plan"));
        assert!(!stdout.contains("automation: running consume-to-mempalace"));
        assert!(!stdout.contains("automation: finished consume-to-mempalace"));
    }

    #[test]
    fn export_outbox_from_uses_consumer_cursor_with_last_id_floor() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let repo = SqliteRepository::open(&db_path).unwrap();

        repo.append_outbox(&WikiEvent::legacy_query_served(
            "first",
            vec!["a".into()],
            OffsetDateTime::now_utc(),
        ))
        .unwrap();
        repo.append_outbox(&WikiEvent::legacy_query_served(
            "second",
            vec!["b".into()],
            OffsetDateTime::now_utc(),
        ))
        .unwrap();
        repo.mark_outbox_processed(1, "mempalace").unwrap();

        let mempalace_export =
            commands::outbox::export_outbox_ndjson_for_consumer_floor(&repo, "mempalace", 0)
                .unwrap();
        assert_eq!(mempalace_export.start_id, 1);
        assert_eq!(mempalace_export.head_id, 2);
        assert!(!mempalace_export.ndjson.contains("first"));
        assert!(mempalace_export.ndjson.contains("second"));

        let fresh_export =
            commands::outbox::export_outbox_ndjson_for_consumer_floor(&repo, "archive", 0).unwrap();
        assert_eq!(fresh_export.start_id, 0);
        assert_eq!(fresh_export.ndjson.lines().count(), 2);

        let override_export =
            commands::outbox::export_outbox_ndjson_for_consumer_floor(&repo, "mempalace", 2)
                .unwrap();
        assert_eq!(override_export.start_id, 2);
        assert_eq!(override_export.head_id, 2);
        assert!(override_export.ndjson.is_empty());
    }

    #[test]
    fn stale_heartbeat_thresholds_classify_expected_levels() {
        let now = OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap();
        let thresholds = test_automation_health_thresholds();
        let green = sample_record(AutomationRunStatus::Running, now - Duration::hours(1));
        let yellow = sample_record(AutomationRunStatus::Running, now - Duration::hours(8));
        let red = sample_record(AutomationRunStatus::Running, now - Duration::hours(36));
        let finished = sample_record(AutomationRunStatus::Succeeded, now - Duration::hours(36));

        assert_eq!(
            classify_stale_heartbeat(&green, now, thresholds),
            AutomationHealthLevel::Green
        );
        assert_eq!(
            classify_stale_heartbeat(&yellow, now, thresholds),
            AutomationHealthLevel::Yellow
        );
        assert_eq!(
            classify_stale_heartbeat(&red, now, thresholds),
            AutomationHealthLevel::Red
        );
        assert_eq!(
            classify_stale_heartbeat(&finished, now, thresholds),
            AutomationHealthLevel::Green
        );
    }

    #[test]
    fn consecutive_failure_thresholds_classify_expected_levels() {
        let thresholds = test_automation_health_thresholds();
        assert_eq!(
            classify_consecutive_failures(0, thresholds),
            AutomationHealthLevel::Green
        );
        assert_eq!(
            classify_consecutive_failures(2, thresholds),
            AutomationHealthLevel::Yellow
        );
        assert_eq!(
            classify_consecutive_failures(3, thresholds),
            AutomationHealthLevel::Red
        );
    }

    #[test]
    fn backlog_thresholds_classify_expected_levels() {
        let thresholds = test_automation_health_thresholds();
        assert_eq!(
            classify_backlog(0, thresholds),
            AutomationHealthLevel::Green
        );
        assert_eq!(
            classify_backlog(25, thresholds),
            AutomationHealthLevel::Yellow
        );
        assert_eq!(
            classify_backlog(120, thresholds),
            AutomationHealthLevel::Red
        );
    }

    #[test]
    fn health_report_render_includes_manual_action_and_failures() {
        let report = AutomationHealthReport {
            level: AutomationHealthLevel::Red,
            issues: vec![AutomationHealthIssue {
                level: AutomationHealthLevel::Red,
                target: "lint".into(),
                code: "consecutive-failures",
                detail: "consecutive_failures=3".into(),
            }],
            outbox: OutboxStats {
                head_id: 10,
                total_events: 10,
                unprocessed_events: 4,
            },
            db_integrity: "ok".into(),
            progress: OutboxConsumerProgress {
                consumer_tag: "mempalace".into(),
                acked_up_to_id: Some(6),
                acked_at: None,
                backlog_events: 4,
            },
            failures: vec![AutomationJobFailureSummary {
                job_name: "lint".into(),
                consecutive_failures: 3,
                latest_failure: Some(sample_record(
                    AutomationRunStatus::Failed,
                    OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap(),
                )),
            }],
        };

        let rendered = render_automation_health_report(&report, "mempalace");
        assert!(rendered.contains("automation health: status=red"));
        assert!(rendered.contains("code=consecutive-failures"));
        assert!(rendered.contains("job=lint consecutive_failures=3"));
        assert!(rendered.contains("manual_action=investigate_and_fix_before_next_daily_run"));
    }

    #[test]
    fn env_vars_override_health_thresholds() {
        // Use unique env-var values that differ from every default so the test
        // is unambiguous even if run in parallel with other tests.
        std::env::set_var("WIKI_HEALTH_BACKLOG_YELLOW", "7");
        std::env::set_var("WIKI_HEALTH_BACKLOG_RED", "14");
        std::env::set_var("WIKI_HEALTH_FAIL_YELLOW", "5");
        std::env::set_var("WIKI_HEALTH_FAIL_RED", "10");
        std::env::set_var("WIKI_HEALTH_STALE_YELLOW_HOURS", "3");
        std::env::set_var("WIKI_HEALTH_STALE_RED_HOURS", "9");

        let t = automation_health_thresholds();
        assert_eq!(t.backlog_yellow, 7);
        assert_eq!(t.backlog_red, 14);
        assert_eq!(t.consecutive_failures_yellow, 5);
        assert_eq!(t.consecutive_failures_red, 10);
        assert_eq!(t.stale_heartbeat_yellow, Duration::hours(3));
        assert_eq!(t.stale_heartbeat_red, Duration::hours(9));

        // Restore defaults so other tests in the same process are not affected.
        for key in &[
            "WIKI_HEALTH_BACKLOG_YELLOW",
            "WIKI_HEALTH_BACKLOG_RED",
            "WIKI_HEALTH_FAIL_YELLOW",
            "WIKI_HEALTH_FAIL_RED",
            "WIKI_HEALTH_STALE_YELLOW_HOURS",
            "WIKI_HEALTH_STALE_RED_HOURS",
        ] {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn gap_empty_db_no_panic() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let repo = SqliteRepository::open(&db_path).unwrap();
        let schema = DomainSchema::permissive_default();
        let eng = LlmWikiEngine::load_from_repo(schema.clone(), &repo, NoopWikiHook).unwrap();
        let viewer = Scope::Private {
            agent_id: "cli".into(),
        };
        let findings = eng.run_gap_scan(Some(&viewer), 2);
        assert!(findings.is_empty(), "空库不应该有 gap");
    }

    #[test]
    fn gap_reports_findings() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let repo = SqliteRepository::open(&db_path).unwrap();
        let schema = DomainSchema::permissive_default();
        let mut eng = LlmWikiEngine::load_from_repo(schema.clone(), &repo, NoopWikiHook).unwrap();
        let viewer = Scope::Private {
            agent_id: "cli".into(),
        };

        // 添加一条 claim，但没有 page 引用它，会触发 gap.missing_xref
        eng.file_claim(
            "项目使用 Redis 进行缓存",
            Scope::Private {
                agent_id: "cli".into(),
            },
            MemoryTier::Semantic,
            "test",
        );

        let findings = eng.run_gap_scan(Some(&viewer), 2);
        assert!(!findings.is_empty(), "应该检测到 gap");
        assert!(
            findings.iter().any(|f| f.code == "gap.missing_xref"),
            "应该检测到 missing_xref"
        );

        // 测试 markdown 报告输出
        let md = gap_report_markdown(&findings);
        assert!(md.contains("# Gap Report"));
        assert!(md.contains("gap.missing_xref"));

        // 测试写入报告文件
        let wiki_dir = dir.path().join("wiki");
        std::fs::create_dir_all(&wiki_dir).unwrap();
        let path = write_gap_report(&wiki_dir, "gap-test", &findings).unwrap();
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("gap.missing_xref"));
    }

    #[test]
    fn fix_empty_db_no_panic() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let repo = SqliteRepository::open(&db_path).unwrap();
        let schema = DomainSchema::permissive_default();
        let mut eng = LlmWikiEngine::load_from_repo(schema.clone(), &repo, NoopWikiHook).unwrap();
        let viewer = Scope::Private {
            agent_id: "cli".into(),
        };
        assert!(
            run_fix_job(&mut eng, &repo, &viewer, false, None, &schema, false, false, false)
                .is_ok()
        );
        assert!(
            run_fix_job(&mut eng, &repo, &viewer, false, None, &schema, true, false, true).is_ok()
        );
        assert!(
            run_fix_job(&mut eng, &repo, &viewer, false, None, &schema, false, true, false).is_ok()
        );
    }

    #[test]
    fn fix_dry_run_does_not_modify() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let repo = SqliteRepository::open(&db_path).unwrap();
        let schema = DomainSchema::permissive_default();
        let mut eng = LlmWikiEngine::load_from_repo(schema.clone(), &repo, NoopWikiHook).unwrap();
        let viewer = Scope::Private {
            agent_id: "cli".into(),
        };

        // 创建一页空标题页面，会触发 page.empty_title → Auto
        let page = WikiPage::new("", "body", viewer.clone());
        let pid = page.id;
        eng.store.pages.insert(pid, page);

        let before = eng.store.pages.get(&pid).unwrap().title.clone();

        run_fix_job(
            &mut eng, &repo, &viewer, false, None, &schema, true, false, true,
        )
        .unwrap();

        let after = eng.store.pages.get(&pid).unwrap().title.clone();
        assert_eq!(before, after, "dry_run 不应修改页面");
    }

    #[test]
    fn fix_outputs_auto_and_manual() {
        // 构造混合 findings，验证 map_findings_to_fixes 同时产出 Auto 和 Manual
        let lint_auto = wiki_core::LintFinding {
            code: "page.empty_title".into(),
            message: "wiki page has empty title".into(),
            severity: wiki_core::LintSeverity::Error,
            subject: Some("00000000-0000-0000-0000-000000000001".into()),
        };
        let lint_manual = wiki_core::LintFinding {
            code: "page.orphan".into(),
            message: "page has no inbound wikilinks".into(),
            severity: wiki_core::LintSeverity::Info,
            subject: Some("00000000-0000-0000-0000-000000000002".into()),
        };
        let fixes = map_findings_to_fixes(&[lint_auto, lint_manual], &[]);
        assert!(fixes.iter().any(|f| f.fix_type == FixActionType::Auto));
        assert!(fixes.iter().any(|f| f.fix_type == FixActionType::Manual));
    }

    #[test]
    fn fix_auto_only_filters_correctly() {
        let lint_auto = wiki_core::LintFinding {
            code: "page.incomplete".into(),
            message: "页面缺少必需段落：定义".into(),
            severity: wiki_core::LintSeverity::Warn,
            subject: Some("00000000-0000-0000-0000-000000000001".into()),
        };
        let lint_manual = wiki_core::LintFinding {
            code: "page.orphan".into(),
            message: "page has no inbound wikilinks".into(),
            severity: wiki_core::LintSeverity::Info,
            subject: Some("00000000-0000-0000-0000-000000000002".into()),
        };
        let mut fixes = map_findings_to_fixes(&[lint_auto, lint_manual], &[]);
        fixes.retain(|f| f.fix_type == FixActionType::Auto);
        assert_eq!(fixes.len(), 1);
        assert_eq!(fixes[0].fix_type, FixActionType::Auto);
        assert_eq!(fixes[0].code, "page.incomplete");
    }

    #[test]
    fn fix_write_applies_append_sections() {
        // 构造：一个 Concept page 缺少"关键要点"段落
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let repo = SqliteRepository::open(&db_path).unwrap();
        let schema = DomainSchema::permissive_default();
        let mut eng = LlmWikiEngine::load_from_repo(schema.clone(), &repo, NoopWikiHook).unwrap();
        let viewer = Scope::Private {
            agent_id: "cli".into(),
        };

        // 创建 Concept 页面，只包含"定义"段落，缺少"关键要点"和"来源引用"
        let page = WikiPage::new("测试页面", "## 定义\n\n这是定义段。\n", viewer.clone())
            .with_entry_type(wiki_core::EntryType::Concept);
        let pid = page.id;
        eng.store.pages.insert(pid, page);

        // 执行：run_fix_job(dry_run=false, auto_only=false, write=true)
        run_fix_job(
            &mut eng, &repo, &viewer, false, None, &schema, false, false, true,
        )
        .unwrap();

        // 断言：page markdown 末尾追加了 "## 关键要点\n\n（待补充）\n\n"
        let page = eng.store.pages.get(&pid).unwrap();
        assert!(
            page.markdown.contains("## 关键要点"),
            "markdown 应包含新追加的关键要点段落"
        );
        assert!(
            page.markdown.contains("（待补充）"),
            "markdown 应包含占位符文本"
        );
    }

    #[test]
    fn fix_write_applies_set_title_from_first_line() {
        // 创建空标题 page，markdown 首行是 ## 某个标题，执行 fix write，验证 title 被设为 某个标题
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let repo = SqliteRepository::open(&db_path).unwrap();
        let schema = DomainSchema::permissive_default();
        let mut eng = LlmWikiEngine::load_from_repo(schema.clone(), &repo, NoopWikiHook).unwrap();
        let viewer = Scope::Private {
            agent_id: "cli".into(),
        };

        // 创建空标题页面，markdown 首行是二级标题
        let page = WikiPage::new("", "## 某个标题\n\n正文内容\n", viewer.clone());
        let pid = page.id;
        eng.store.pages.insert(pid, page);

        // 执行：run_fix_job(dry_run=false, auto_only=false, write=true)
        run_fix_job(
            &mut eng, &repo, &viewer, false, None, &schema, false, false, true,
        )
        .unwrap();

        // 断言：page 标题应被设为"某个标题"
        let page = eng.store.pages.get(&pid).unwrap();
        assert_eq!(page.title, "某个标题", "空标题应从 markdown 首行提取");
    }

    #[test]
    fn fix_write_does_not_overwrite_non_empty_title() {
        // 安全：当页面已有非空标题时，SetTitle 不应覆盖
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let repo = SqliteRepository::open(&db_path).unwrap();
        let schema = DomainSchema::permissive_default();
        let mut eng = LlmWikiEngine::load_from_repo(schema.clone(), &repo, NoopWikiHook).unwrap();
        let viewer = Scope::Private {
            agent_id: "cli".into(),
        };

        let page = WikiPage::new("现有标题", "## 新标题\n\n正文\n", viewer.clone());
        let pid = page.id;
        eng.store.pages.insert(pid, page);

        // 直接调用 apply_auto_fixes 并传入强制 SetTitle
        let fix = FixAction {
            code: "page.empty_title".into(),
            fix_type: FixActionType::Auto,
            description: "test".into(),
            subject: Some(pid.0.to_string()),
            subject_label: None,
            patch: Some(FixPatch::SetTitle {
                title: "新标题".into(),
            }),
        };
        let modified = apply_auto_fixes(&mut eng, &[fix]);
        assert_eq!(modified, 0, "已有标题不应被覆盖");
        assert_eq!(eng.store.pages.get(&pid).unwrap().title, "现有标题");
    }

    #[test]
    fn fix_write_does_not_duplicate_existing_sections() {
        // 安全：AppendSections 不应重复追加已存在的段落
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let repo = SqliteRepository::open(&db_path).unwrap();
        let schema = DomainSchema::permissive_default();
        let mut eng = LlmWikiEngine::load_from_repo(schema.clone(), &repo, NoopWikiHook).unwrap();
        let viewer = Scope::Private {
            agent_id: "cli".into(),
        };

        let page = WikiPage::new("测试", "## 定义\n\n已有内容\n", viewer.clone());
        let pid = page.id;
        eng.store.pages.insert(pid, page);

        let fix = FixAction {
            code: "page.incomplete".into(),
            fix_type: FixActionType::Auto,
            description: "test".into(),
            subject: Some(pid.0.to_string()),
            subject_label: None,
            patch: Some(FixPatch::AppendSections {
                sections: vec!["定义".into(), "新段落".into()],
            }),
        };
        let modified = apply_auto_fixes(&mut eng, &[fix]);
        assert_eq!(modified, 1);
        let md = &eng.store.pages.get(&pid).unwrap().markdown;
        assert_eq!(md.matches("## 定义").count(), 1, "已存在的段落不应重复追加");
        assert!(md.contains("## 新段落"), "新段落应该被追加");
    }

    #[test]
    fn query_to_page_uses_page_contract_qa_type() {
        let schema = DomainSchema::permissive_default();
        let page = query_to_page(
            "测试查询",
            "这是问题",
            &[("doc1".into(), 0.9)],
            Scope::Private {
                agent_id: "test".into(),
            },
            None,
            &schema,
        );
        assert_eq!(page.entry_type, Some(EntryType::Qa));
        assert_eq!(page.status, EntryStatus::Approved);
    }

    #[test]
    fn query_to_page_custom_entry_type() {
        let schema = DomainSchema::permissive_default();
        let page = query_to_page(
            "测试查询",
            "这是问题",
            &[],
            Scope::Private {
                agent_id: "test".into(),
            },
            Some(EntryType::Concept),
            &schema,
        );
        assert_eq!(page.entry_type, Some(EntryType::Concept));
        assert_eq!(page.status, EntryStatus::Draft);
    }

    #[test]
    fn query_to_page_has_question_and_answer_sections() {
        let schema = DomainSchema::permissive_default();
        let page = query_to_page(
            "测试查询",
            "这是问题",
            &[("doc1".into(), 0.9), ("doc2".into(), 0.8)],
            Scope::Private {
                agent_id: "test".into(),
            },
            None,
            &schema,
        );
        assert!(page.markdown.contains("## 问题\n\n这是问题"));
        assert!(page.markdown.contains("## 回答"));
        assert!(page.markdown.contains("`doc1` score=0.900000"));
        assert!(page.markdown.contains("`doc2` score=0.800000"));
    }

    #[test]
    fn qa_command_creates_qa_type_page() {
        let schema = DomainSchema::permissive_default();
        let question = "什么是 Rust？".to_string();
        let answer = "Rust 是一门系统编程语言。".to_string();
        let et = parse_entry_type_opt(&None)
            .unwrap_or(None)
            .unwrap_or(EntryType::Qa);
        let status = initial_status_for(Some(&et), &schema);

        let page = PageContract::new(&question, et)
            .with_confidence(Confidence::default())
            .with_source("qa")
            .with_section("问题", &question)
            .with_section("回答", &answer)
            .into_page(
                Scope::Private {
                    agent_id: "test".into(),
                },
                status,
            );

        assert_eq!(page.entry_type, Some(EntryType::Qa));
        assert_eq!(page.status, EntryStatus::Approved);
        assert!(page.markdown.contains("## 问题"));
        assert!(page.markdown.contains("## 回答"));
    }

    #[test]
    fn qa_command_custom_entry_type() {
        let schema = DomainSchema::permissive_default();
        let question = "问题".to_string();
        let answer = "回答".to_string();
        let et = parse_entry_type_opt(&Some("concept".into()))
            .unwrap()
            .unwrap_or(EntryType::Qa);
        let status = initial_status_for(Some(&et), &schema);

        let page = PageContract::new(&question, et)
            .with_confidence(Confidence::default())
            .with_source("qa")
            .with_section("问题", &question)
            .with_section("回答", &answer)
            .into_page(
                Scope::Private {
                    agent_id: "test".into(),
                },
                status,
            );

        assert_eq!(page.entry_type, Some(EntryType::Concept));
    }

    #[test]
    fn synthesis_command_creates_synthesis_type_page() {
        let schema = DomainSchema::permissive_default();
        let topic = "Rust 异步编程研究".to_string();
        let body_text = "综合分析正文内容。".to_string();
        let et = EntryType::Synthesis;
        let status = initial_status_for(Some(&et), &schema);

        let page = PageContract::new(&topic, et)
            .with_confidence(Confidence::default())
            .with_source("synthesis")
            .with_section("研究问题", &topic)
            .with_section("综合分析", &body_text)
            .into_page(
                Scope::Private {
                    agent_id: "test".into(),
                },
                status,
            );

        assert_eq!(page.entry_type, Some(EntryType::Synthesis));
        assert_eq!(page.status, EntryStatus::Draft);
        assert!(page.markdown.contains("## 研究问题"));
        assert!(page.markdown.contains("## 综合分析"));
    }

    #[test]
    fn query_without_palace_db_uses_storage_ports() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let repo = SqliteRepository::open(&db_path).unwrap();
        let schema = DomainSchema::permissive_default();
        let mut eng = LlmWikiEngine::load_from_repo(schema.clone(), &repo, NoopWikiHook).unwrap();
        let viewer = Scope::Private {
            agent_id: "cli".into(),
        };

        let claim_id = eng.file_claim(
            "Rust 是一门系统编程语言",
            viewer.clone(),
            MemoryTier::Semantic,
            "test",
        );
        eng.save_to_repo(&repo).unwrap();
        eng.store.claims.clear();

        let ctx = QueryContext::new("Rust 编程语言")
            .with_rrf_k(60.0)
            .with_per_stream_limit(10)
            .with_viewer_scope(viewer.clone());
        let ranked = run_fusion_query(
            None,
            "wiki",
            &repo,
            &eng,
            &viewer,
            &ctx,
            OffsetDateTime::now_utc(),
            None,
            None,
        );
        assert!(!ranked.is_empty(), "应该能检索到结果");
        assert!(
            ranked
                .iter()
                .any(|(id, _)| id == &format!("claim:{}", claim_id.0)),
            "结果应来自已持久化的 storage-backed search ports"
        );
    }

    #[test]
    fn query_with_invalid_palace_db_falls_back_to_storage_ports() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let repo = SqliteRepository::open(&db_path).unwrap();
        let schema = DomainSchema::permissive_default();
        let mut eng = LlmWikiEngine::load_from_repo(schema.clone(), &repo, NoopWikiHook).unwrap();
        let viewer = Scope::Private {
            agent_id: "cli".into(),
        };

        let claim_id = eng.file_claim(
            "Rust 是一门系统编程语言",
            viewer.clone(),
            MemoryTier::Semantic,
            "test",
        );
        eng.save_to_repo(&repo).unwrap();
        eng.store.claims.clear();

        // 用一个目录路径作为 palace_db，让 MempalaceSearchPorts::open 失败
        let palace_dir = dir.path().join("palace_dir");
        std::fs::create_dir(&palace_dir).unwrap();

        let result = MempalaceSearchPorts::open(&palace_dir, Some("wiki".into()));
        assert!(result.is_err(), "目录路径应该无法打开为 palace DB");

        // 回退到纯 wiki 检索，确保不崩溃
        let ctx = QueryContext::new("Rust 编程语言")
            .with_rrf_k(60.0)
            .with_per_stream_limit(10)
            .with_viewer_scope(viewer.clone());
        let ranked = run_fusion_query(
            Some(palace_dir.to_str().unwrap()),
            "wiki",
            &repo,
            &eng,
            &viewer,
            &ctx,
            OffsetDateTime::now_utc(),
            None,
            None,
        );
        assert!(!ranked.is_empty(), "回退后应该能检索到结果");
        assert!(
            ranked
                .iter()
                .any(|(id, _)| id == &format!("claim:{}", claim_id.0)),
            "invalid palace fallback should still use storage-backed wiki ports"
        );
    }

    #[test]
    fn composite_search_ports_can_be_constructed() {
        // 验证 CompositeSearchPorts 导入正确且可构建
        let composite = CompositeSearchPorts::new(vec![], FusionConfig::default());
        assert!(composite.bm25_ranked_ids("q", 10).is_empty());
    }

    #[test]
    fn explain_without_palace_db_uses_storage_ports() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let repo = SqliteRepository::open(&db_path).unwrap();
        let schema = DomainSchema::permissive_default();
        let mut eng = LlmWikiEngine::load_from_repo(schema.clone(), &repo, NoopWikiHook).unwrap();
        let viewer = Scope::Private {
            agent_id: "cli".into(),
        };

        let claim_id = eng.file_claim(
            "Rust 是一门系统编程语言",
            viewer.clone(),
            MemoryTier::Semantic,
            "test",
        );
        eng.save_to_repo(&repo).unwrap();
        eng.store.claims.clear();

        let ctx = QueryContext::new("Rust 编程语言")
            .with_rrf_k(60.0)
            .with_per_stream_limit(10)
            .with_viewer_scope(viewer.clone());
        let ports = build_wiki_search_ports(&repo, &eng, &viewer);
        let ranked = eng.query_ranked_with_ports(
            &ctx,
            OffsetDateTime::now_utc(),
            ports.as_ref(),
            None,
            None,
        );
        assert!(!ranked.is_empty(), "explain 应该能检索到结果");
        assert!(
            ranked
                .iter()
                .any(|(id, _)| id == &format!("claim:{}", claim_id.0)),
            "explain 结果应来自 storage-backed search ports"
        );
    }

    #[test]
    fn explain_with_invalid_palace_db_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let repo = SqliteRepository::open(&db_path).unwrap();
        let schema = DomainSchema::permissive_default();
        let mut eng = LlmWikiEngine::load_from_repo(schema.clone(), &repo, NoopWikiHook).unwrap();
        let viewer = Scope::Private {
            agent_id: "cli".into(),
        };

        eng.file_claim(
            "Rust 是一门系统编程语言",
            viewer.clone(),
            MemoryTier::Semantic,
            "test",
        );

        // 用一个目录路径作为 palace_db，让 MempalaceSearchPorts::open 失败
        let palace_dir = dir.path().join("palace_dir");
        std::fs::create_dir(&palace_dir).unwrap();

        let result = MempalaceSearchPorts::open(&palace_dir, Some("wiki".into()));
        assert!(result.is_err(), "目录路径应该无法打开为 palace DB");

        // 回退到纯 wiki 检索，确保不崩溃
        let ctx = QueryContext::new("Rust 编程语言")
            .with_rrf_k(60.0)
            .with_per_stream_limit(10)
            .with_viewer_scope(viewer.clone());
        let ports = InMemorySearchPorts::new(&eng.store, Some(viewer.clone()));
        let ranked =
            eng.query_ranked_with_ports(&ctx, OffsetDateTime::now_utc(), &ports, None, None);
        assert!(!ranked.is_empty(), "explain 回退后应该能检索到结果");
    }
}
