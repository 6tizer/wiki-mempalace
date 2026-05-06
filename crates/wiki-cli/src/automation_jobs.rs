#![allow(clippy::too_many_arguments)]

use std::collections::{BTreeSet, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};

use time::OffsetDateTime;
use wiki_core::{
    ClaimId, Confidence, DomainSchema, EntryType, EvidenceFixerPlan, FixAction, FixActionType,
    FixPatch, GapFinding, GapSeverity, GovernanceDuplicateGroup, GovernanceScanReport,
    PageContract, PageId, Scope, SourceId, StrategyExecutionAction, StrategyExecutionDryRunStatus,
    StrategyExecutionPlan, WikiPage,
};
use wiki_kernel::{
    apply_evidence_fixer_plan, build_evidence_fixer_plan, discover_synthesis_candidates,
    duplicate_web_verification_key, finalize_consumed_page, initial_status_for,
    map_findings_to_fixes, run_governance_scan, write_lint_report, write_projection,
    EvidenceFixerApplyOptions, EvidenceFixerPlanOptions, GovernanceScanOptions, InMemoryStore,
    LlmWikiEngine, NoopWikiHook, SynthesisDiscoveryOptions,
};
use wiki_mempalace_bridge::{
    consume_outbox_ndjson_with_resolver_and_stats, LiveMempalaceSink, MempalaceError,
    MempalaceWikiSink, OutboxDispatchStats, OutboxResolver,
};
use wiki_storage::{EmbeddingWrite, SqliteRepository, WikiRepository};

use crate::automation::{
    automation_job_name, automation_job_spec, automation_run_daily_jobs, format_automation_record,
    latest_automation_run_or_error, run_automation_job, run_automation_plan, AutomationHeartbeat,
    AutomationJob,
};
use crate::cli_utils::{
    default_fixer_report_dir, default_governance_report_dir, default_synthesis_report_dir, env_or,
    parse_scope, resolve_wiki_relative_path, timestamp_slug, DEFAULT_MEMPALACE_CONSUMER_TAG,
};
use crate::commands;
use crate::strategy_render::{
    strategy_execution_action_kind_name, strategy_executor_report_prefix,
    StrategyExecutorActionStatus, StrategyExecutorApplyActionReport, StrategyExecutorApplyReport,
    StrategyExecutorApplySummary, StrategyExecutorRunMode,
};
use crate::{Cli, ExecutorAllow, NotionDbTarget, NotionSyncTagPolicy};

// ---------------------------------------------------------------------------
// Research synthesis compose
// ---------------------------------------------------------------------------

pub(crate) struct ResearchSynthesisComposeInputs {
    pub(crate) web_evidence: Option<PathBuf>,
    pub(crate) draft_json: Option<PathBuf>,
    pub(crate) verifier_json: Option<PathBuf>,
    pub(crate) internal_only: bool,
    pub(crate) allow_private_web_search: bool,
    pub(crate) apply: bool,
}

// ---------------------------------------------------------------------------
// EngineResolver & CliMempalaceSink
// ---------------------------------------------------------------------------

/// 用当前 in-memory store 反解 `ClaimUpserted` / `SourceIngested` 的 payload + scope。
pub(crate) struct EngineResolver<'a> {
    pub(crate) store: &'a InMemoryStore,
}

impl<'a> OutboxResolver for EngineResolver<'a> {
    fn claim(&self, id: wiki_core::ClaimId) -> Option<wiki_core::Claim> {
        self.store.claims.get(&id).cloned()
    }

    fn source_scope(&self, id: wiki_core::SourceId) -> Option<Scope> {
        self.store.sources.get(&id).map(|s| s.scope.clone())
    }

    fn page(&self, id: wiki_core::PageId) -> Option<wiki_core::WikiPage> {
        self.store.pages.get(&id).cloned()
    }
}

pub(crate) struct CliMempalaceSink;

impl MempalaceWikiSink for CliMempalaceSink {
    fn on_claim_upserted(&self, claim: &wiki_core::Claim) -> Result<(), MempalaceError> {
        // resolver 路径：打印 id + 文本前缀，证明 payload 已被还原；真正写入 palace 由
        // live sink 在 wiki-mempalace-bridge 的 `live` feature 下完成。
        let preview: String = claim.text.chars().take(80).collect();
        println!("mempalace claim_upserted {} {}", claim.id.0, preview);
        Ok(())
    }

    fn on_claim_event(&self, claim_id: wiki_core::ClaimId) -> Result<(), MempalaceError> {
        // 仅在 resolver 无法解析 claim 时走到这里（悬挂事件）。
        println!("mempalace claim_upserted(unresolved) {}", claim_id.0);
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

// ---------------------------------------------------------------------------
// Notion DB constants
// ---------------------------------------------------------------------------

/// Notion DB configurations: (slug, Notion DB UUID)
pub(crate) const NOTION_DB_X_BOOKMARK: (&str, &str) =
    ("x_bookmark", "0d305291-2a5d-426c-8db8-903ed5bb7ddb");
pub(crate) const NOTION_DB_WECHAT: (&str, &str) =
    ("wechat", "16470107-4b68-810a-bc81-f90795cc29ad");

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

pub(crate) fn maybe_sync_projection(
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

pub(crate) fn run_research_synthesis_compose(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    viewer: &Scope,
    sync_wiki: bool,
    wiki_root: Option<&std::path::Path>,
    llm_config_path: &std::path::Path,
    discovery_report: &wiki_core::SynthesisDiscoveryReport,
    candidate_id: &str,
    inputs: ResearchSynthesisComposeInputs,
) -> Result<crate::research_synthesis::SynthesisComposeReport, Box<dyn std::error::Error>> {
    let candidate =
        crate::research_synthesis::select_candidate(discovery_report, candidate_id)?.clone();
    let internal_evidence =
        crate::research_synthesis::build_internal_evidence(&eng.store, viewer, &candidate);
    let mut extra_blockers = Vec::new();
    let private_web_blocked = matches!(viewer, Scope::Private { .. })
        && !inputs.internal_only
        && !inputs.allow_private_web_search;
    if private_web_blocked {
        extra_blockers.push(
            "private viewer scope requires --allow-private-web-search for web research".into(),
        );
    }

    let web_runs = match inputs.web_evidence {
        Some(path) => {
            let path = resolve_wiki_relative_path(wiki_root, path);
            crate::research_synthesis::read_web_runs(&path)?
        }
        None if inputs.internal_only || private_web_blocked => Vec::new(),
        None => match crate::research_synthesis::run_web_research(
            llm_config_path,
            &candidate,
            &internal_evidence,
        ) {
            Ok(runs) => runs,
            Err(err) => {
                extra_blockers.push(format!("web research failed: {err}"));
                Vec::new()
            }
        },
    };

    let draft = match inputs.draft_json {
        Some(path) => {
            let path = resolve_wiki_relative_path(wiki_root, path);
            Some(crate::research_synthesis::read_draft(&path)?)
        }
        None if extra_blockers.is_empty() => {
            match crate::research_synthesis::generate_draft_with_llm(
                llm_config_path,
                &candidate,
                &internal_evidence,
                &web_runs,
            ) {
                Ok(draft) => Some(draft),
                Err(err) => {
                    extra_blockers.push(format!("synthesis_writer failed: {err}"));
                    None
                }
            }
        }
        None => None,
    };

    let verifier = match inputs.verifier_json {
        Some(path) => {
            let path = resolve_wiki_relative_path(wiki_root, path);
            Some(crate::research_synthesis::read_verifier(&path)?)
        }
        None if extra_blockers.is_empty() => {
            if let Some(draft) = &draft {
                match crate::research_synthesis::verify_with_llm(
                    llm_config_path,
                    draft,
                    &candidate,
                    &internal_evidence,
                    &web_runs,
                ) {
                    Ok(verdict) => Some(verdict),
                    Err(err) => {
                        extra_blockers.push(format!("synthesis_verifier failed: {err}"));
                        None
                    }
                }
            } else {
                None
            }
        }
        None => None,
    };

    let now = OffsetDateTime::now_utc();
    let mut report = crate::research_synthesis::build_compose_report(
        crate::research_synthesis::synthesis_compose_report_prefix(now),
        now,
        discovery_report,
        candidate.clone(),
        internal_evidence.clone(),
        web_runs.clone(),
        draft.clone(),
        verifier.clone(),
        false,
        None,
        inputs.internal_only,
    );
    add_research_synthesis_blockers(&mut report, extra_blockers);
    if report.status == crate::research_synthesis::SynthesisComposeStatus::Ready && inputs.apply {
        let draft = draft.ok_or("ready synthesis compose report missing draft")?;
        let page =
            crate::research_synthesis::build_synthesis_page(&candidate, &draft, viewer.clone());
        let pid = page.id;
        eng.store.pages.insert(pid, page);
        eng.save_to_repo_and_flush_outbox_with_policy(repo, 128, 3)?;
        maybe_sync_projection(sync_wiki, wiki_root, eng)?;
        let now = OffsetDateTime::now_utc();
        report = crate::research_synthesis::build_compose_report(
            crate::research_synthesis::synthesis_compose_report_prefix(now),
            now,
            discovery_report,
            candidate,
            internal_evidence,
            web_runs,
            Some(draft),
            verifier,
            true,
            Some(pid.0.to_string()),
            inputs.internal_only,
        );
    }
    Ok(report)
}

pub(crate) fn add_research_synthesis_blockers(
    report: &mut crate::research_synthesis::SynthesisComposeReport,
    blockers: Vec<String>,
) {
    if blockers.is_empty() {
        return;
    }
    report.blockers.extend(blockers);
    report.blockers.sort();
    report.blockers.dedup();
    report.status = crate::research_synthesis::SynthesisComposeStatus::Blocked;
    report.page_id = None;
}

pub(crate) fn save_to_repo_and_flush_outbox_with_embeddings(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    embeddings: Vec<EmbeddingWrite>,
) -> Result<usize, Box<dyn std::error::Error>> {
    let snapshot = eng.store.to_snapshot(&eng.audits);
    let inserted =
        repo.save_snapshot_and_append_outbox_with_embeddings(&snapshot, &eng.outbox, &embeddings)?;
    eng.outbox.clear();
    Ok(inserted)
}

pub(crate) fn run_lint_job(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    viewer: &Scope,
    sync_wiki: bool,
    wiki_root: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let findings = eng.run_basic_lint("cli", Some(viewer));
    eng.save_to_repo_and_flush_outbox_with_policy(repo, 128, 3)?;
    if let Some(root) = wiki_root {
        let report = write_lint_report(root, &format!("lint-{}", timestamp_slug()), &findings)?;
        println!("lint_report={}", report.display());
    }
    maybe_sync_projection(sync_wiki, wiki_root, eng)?;
    for f in findings {
        println!("{:?}\t{}\t{}", f.severity, f.code, f.message);
    }
    Ok(())
}

/// 将 GapFinding 列表渲染为 markdown 字符串。
///
/// 共享函数：`write_gap_report`（写文件）和 `gap_report_markdown`（写 page）都调用它。
pub(crate) fn render_gap_markdown(findings: &[GapFinding]) -> String {
    let severity_order = [GapSeverity::High, GapSeverity::Medium, GapSeverity::Low];
    let mut grouped: std::collections::BTreeMap<&str, Vec<&GapFinding>> =
        std::collections::BTreeMap::new();
    for f in findings {
        let key = match f.severity {
            GapSeverity::High => "high",
            GapSeverity::Medium => "medium",
            GapSeverity::Low => "low",
        };
        grouped.entry(key).or_default().push(f);
    }
    let mut md = String::from("# Gap Report\n\n");
    md.push_str(&format!("- total gaps: `{}`\n\n", findings.len()));
    for sev in &severity_order {
        let key = match sev {
            GapSeverity::High => "high",
            GapSeverity::Medium => "medium",
            GapSeverity::Low => "low",
        };
        if let Some(items) = grouped.get(key) {
            md.push_str(&format!("## {key}\n\n"));
            for item in items {
                let subject_info = match (&item.subject, &item.subject_label) {
                    (Some(s), Some(l)) => format!(" (subject={s}, label={l})"),
                    (Some(s), None) => format!(" (subject={s})"),
                    (None, Some(l)) => format!(" (label={l})"),
                    (None, None) => String::new(),
                };
                md.push_str(&format!(
                    "- `{}` {}{}\n",
                    item.code, item.message, subject_info
                ));
            }
            md.push('\n');
        }
    }
    md
}

/// 生成 gap 报告的 markdown 文件，写入 wiki/reports/gap-{timestamp}.md
pub(crate) fn write_gap_report(
    wiki_root: &std::path::Path,
    report_name: &str,
    findings: &[GapFinding],
) -> std::io::Result<std::path::PathBuf> {
    use std::fs;

    let reports_dir = wiki_root.join("reports");
    fs::create_dir_all(&reports_dir)?;
    let filename = if report_name.ends_with(".md") {
        report_name.to_string()
    } else {
        format!("{report_name}.md")
    };
    let out = reports_dir.join(filename);
    let md = render_gap_markdown(findings);
    fs::write(&out, md)?;
    Ok(out)
}

pub(crate) fn run_gap_job(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    viewer: &Scope,
    sync_wiki: bool,
    wiki_root: Option<&std::path::Path>,
    low_coverage_threshold: usize,
    write_page: bool,
    schema: &DomainSchema,
) -> Result<(), Box<dyn std::error::Error>> {
    let findings = eng.run_gap_scan(Some(viewer), low_coverage_threshold);
    eng.save_to_repo_and_flush_outbox_with_policy(repo, 128, 3)?;

    let report_md = gap_report_markdown(&findings);

    if let Some(root) = wiki_root {
        let report_path = write_gap_report(root, &format!("gap-{}", timestamp_slug()), &findings)?;
        println!("gap_report={}", report_path.display());
    }

    if write_page {
        let title = format!("gap-report-{}", timestamp_slug());
        let page = WikiPage::new(title, report_md, viewer.clone());
        let pid = page.id;
        eng.store.pages.insert(pid, page);
        if let Some(page) = eng.store.pages.get_mut(&pid) {
            finalize_consumed_page(page, EntryType::LintReport, Confidence::default(), schema);
        }
        eng.save_to_repo_and_flush_outbox_with_policy(repo, 128, 3)?;
    }

    maybe_sync_projection(sync_wiki, wiki_root, eng)?;
    for f in &findings {
        println!("{:?}\t{}\t{}", f.severity, f.code, f.message);
    }
    Ok(())
}

/// 执行 Auto 类型 fix action 的 patch，返回实际修改的 page 数量。
pub(crate) fn apply_auto_fixes(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    fixes: &[FixAction],
) -> usize {
    let mut modified_pages = std::collections::HashSet::new();
    for fix in fixes {
        if fix.fix_type != FixActionType::Auto {
            continue;
        }
        let Some(ref subject) = fix.subject else {
            continue;
        };
        let Ok(pid) = uuid::Uuid::parse_str(subject) else {
            continue;
        };
        let pid = PageId(pid);
        let Some(page) = eng.store.pages.get_mut(&pid) else {
            continue;
        };

        // 优先使用 fix.patch；若 patch 为 None 且 code 是 page.empty_title，
        // 则从 markdown 第一行提取标题作为 fallback。
        let patch = fix.patch.clone().or_else(|| {
            if fix.code == "page.empty_title" {
                page.markdown
                    .lines()
                    .find(|line| {
                        let t = line.trim();
                        !t.is_empty() && !t.starts_with("---")
                    })
                    .map(|line| line.trim().trim_start_matches('#').trim_start().to_string())
                    .filter(|t| !t.is_empty())
                    .map(|title| FixPatch::SetTitle { title })
            } else {
                None
            }
        });

        let Some(patch) = patch else { continue };

        match patch {
            FixPatch::AppendSections { sections } => {
                for section in sections {
                    // 若 markdown 中已存在同名的 ## 标题，则跳过，防止重复追加
                    let heading = format!("## {section}");
                    if page.markdown.contains(&heading) {
                        continue;
                    }
                    page.markdown
                        .push_str(&format!("## {section}\n\n（待补充）\n\n"));
                }
                page.updated_at = OffsetDateTime::now_utc();
                modified_pages.insert(pid);
            }
            FixPatch::SetTitle { title } => {
                // 安全：仅当当前标题为空时才设置，避免覆盖已有标题
                if page.title.trim().is_empty() {
                    page.title = title;
                    page.updated_at = OffsetDateTime::now_utc();
                    modified_pages.insert(pid);
                }
            }
            FixPatch::AddXref { .. } => {
                // 第一版不实现 AddXref 的自动执行
            }
        }
    }
    modified_pages.len()
}

pub(crate) fn collect_auto_fixes_for_executor(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    viewer: &Scope,
) -> Vec<FixAction> {
    let lint_findings = eng.run_basic_lint("cli", Some(viewer));
    let gap_findings = eng.run_gap_scan(Some(viewer), 2);
    let mut fixes = map_findings_to_fixes(&lint_findings, &gap_findings);
    fixes.retain(|fix| fix.fix_type == FixActionType::Auto);
    fixes
}

pub(crate) fn auto_fix_suggestion_reason(fix: &FixAction) -> String {
    format!("Low-risk fixer action is available: {}", fix.description)
}

pub(crate) fn matching_executor_auto_fix<'a>(
    fixes: &'a [FixAction],
    action: &StrategyExecutionAction,
) -> Option<&'a FixAction> {
    let subject = action.subject.as_deref()?;
    fixes.iter().find(|fix| {
        fix.subject.as_deref() == Some(subject)
            && auto_fix_suggestion_reason(fix) == action.suggestion_reason
    })
}

pub(crate) fn build_strategy_executor_apply_report(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    viewer: &Scope,
    sync_wiki: bool,
    wiki_root: Option<&std::path::Path>,
    plan: &StrategyExecutionPlan,
    allow: &[ExecutorAllow],
    apply: bool,
) -> Result<StrategyExecutorApplyReport, Box<dyn std::error::Error>> {
    if apply && allow.is_empty() {
        return Err("--apply requires at least one --allow value".into());
    }
    let generated_at = OffsetDateTime::now_utc();
    let allow_kinds = allow
        .iter()
        .map(|item| item.action_kind())
        .collect::<HashSet<_>>();
    let mut applied_signatures = HashSet::new();
    let mut action_reports = Vec::new();
    let fixes = collect_auto_fixes_for_executor(eng, viewer);

    for action in &plan.actions {
        let (status, reason) = if action.dry_run_status != StrategyExecutionDryRunStatus::WouldApply
        {
            (StrategyExecutorActionStatus::Blocked, action.reason.clone())
        } else if !allow_kinds.contains(&action.action_kind) {
            (
                StrategyExecutorActionStatus::Blocked,
                format!(
                    "action_kind {} is not allowed; pass --allow fix-auto-safe to permit it",
                    strategy_execution_action_kind_name(action.action_kind)
                ),
            )
        } else if action.suggestion_reason.trim().is_empty() {
            (
                StrategyExecutorActionStatus::Blocked,
                "plan action lacks suggestion_reason evidence; regenerate the dry-run plan"
                    .to_string(),
            )
        } else if action.subject.is_none() {
            (
                StrategyExecutorActionStatus::Blocked,
                "plan action has no subject".to_string(),
            )
        } else if let Some(fix) = matching_executor_auto_fix(&fixes, action) {
            let signature = format!(
                "{}\n{}",
                action.subject.as_deref().unwrap_or_default(),
                action.suggestion_reason
            );
            if !applied_signatures.insert(signature) {
                (
                    StrategyExecutorActionStatus::Skipped,
                    "duplicate planned auto fix signature".to_string(),
                )
            } else if apply {
                let modified = apply_auto_fixes(eng, std::slice::from_ref(fix));
                if modified > 0 {
                    (
                        StrategyExecutorActionStatus::Applied,
                        "applied allowlisted auto fix".to_string(),
                    )
                } else {
                    (
                        StrategyExecutorActionStatus::Skipped,
                        "matched auto fix was already a no-op".to_string(),
                    )
                }
            } else {
                (
                    StrategyExecutorActionStatus::WouldApply,
                    "validated allowlisted auto fix; pass --apply to execute".to_string(),
                )
            }
        } else {
            (
                StrategyExecutorActionStatus::Skipped,
                "planned auto fix is no longer present; rerun suggest --executor-plan".to_string(),
            )
        };

        action_reports.push(StrategyExecutorApplyActionReport {
            action_id: action.action_id.clone(),
            suggestion_id: action.suggestion_id.clone(),
            code: action.code.clone(),
            subject: action.subject.clone(),
            action_kind: action.action_kind,
            status,
            reason,
        });
    }

    let summary = StrategyExecutorApplySummary {
        total: action_reports.len(),
        would_apply: action_reports
            .iter()
            .filter(|action| action.status == StrategyExecutorActionStatus::WouldApply)
            .count(),
        applied: action_reports
            .iter()
            .filter(|action| action.status == StrategyExecutorActionStatus::Applied)
            .count(),
        blocked: action_reports
            .iter()
            .filter(|action| action.status == StrategyExecutorActionStatus::Blocked)
            .count(),
        skipped: action_reports
            .iter()
            .filter(|action| action.status == StrategyExecutorActionStatus::Skipped)
            .count(),
    };

    if apply && summary.applied > 0 {
        eng.save_to_repo_and_flush_outbox_with_policy(repo, 128, 3)?;
        maybe_sync_projection(sync_wiki, wiki_root, eng)?;
    }

    Ok(StrategyExecutorApplyReport {
        report_id: strategy_executor_report_prefix(generated_at),
        plan_id: plan.plan_id.clone(),
        source_report_id: plan.source_report_id.clone(),
        generated_at,
        mode: if apply {
            StrategyExecutorRunMode::Apply
        } else {
            StrategyExecutorRunMode::Preflight
        },
        apply_requested: apply,
        allowlist: allow.iter().map(|item| item.name().to_string()).collect(),
        summary,
        actions: action_reports,
    })
}

/// 检测并修复 lint/gap finding，输出修复动作列表。
pub(crate) fn run_fix_job(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    viewer: &Scope,
    sync_wiki: bool,
    wiki_root: Option<&std::path::Path>,
    _schema: &DomainSchema,
    dry_run: bool,
    auto_only: bool,
    write: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let lint_findings = eng.run_basic_lint("cli", Some(viewer));
    let gap_findings = eng.run_gap_scan(Some(viewer), 2);
    let mut fixes = map_findings_to_fixes(&lint_findings, &gap_findings);

    if auto_only {
        fixes.retain(|f| f.fix_type == FixActionType::Auto);
    }

    for fix in &fixes {
        let type_str = match fix.fix_type {
            FixActionType::Auto => "Auto",
            FixActionType::Draft => "Draft",
            FixActionType::Manual => "Manual",
        };
        println!("{}\t{}\t{}", type_str, fix.code, fix.description);
    }

    if write && !dry_run {
        let modified = apply_auto_fixes(eng, &fixes);
        if modified > 0 {
            eng.save_to_repo_and_flush_outbox_with_policy(repo, 128, 3)?;
            maybe_sync_projection(sync_wiki, wiki_root, eng)?;
        }
    }

    Ok(())
}

/// 将 GapFinding 列表渲染为 markdown 字符串（用于 --write-page）
pub(crate) fn gap_report_markdown(findings: &[GapFinding]) -> String {
    render_gap_markdown(findings)
}

pub(crate) fn run_maintenance_job(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    viewer: &Scope,
    sync_wiki: bool,
    wiki_root: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = OffsetDateTime::now_utc();
    eng.apply_confidence_decay_all(now, 30.0);
    let findings = eng.run_basic_lint("cli", Some(viewer));
    let mut promoted = 0u32;
    let claim_ids: Vec<ClaimId> = eng.store.claims.keys().copied().collect();
    for cid in claim_ids {
        if eng.promote_if_qualified(cid, "cli", viewer).is_ok() {
            promoted += 1;
        }
    }
    let pages_marked = eng.mark_stale_pages(now);
    let pages_cleaned = eng.cleanup_expired_pages(now);
    eng.save_to_repo_and_flush_outbox_with_policy(repo, 128, 3)?;
    maybe_sync_projection(sync_wiki, wiki_root, eng)?;
    println!(
        "decay=applied lint_findings={} promoted={promoted} pages_marked_needs_update={pages_marked} pages_auto_cleaned={pages_cleaned}",
        findings.len()
    );
    Ok(())
}

pub(crate) fn mempalace_bank_from_viewer_scope(viewer_scope: &str) -> String {
    match parse_scope(viewer_scope) {
        Scope::Private { agent_id } => agent_id,
        Scope::Shared { team_id } => team_id,
    }
}

pub(crate) fn run_consume_to_mempalace_job(
    eng: &LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    consumer_tag: &str,
    last_id: i64,
    palace_path: Option<&std::path::Path>,
    viewer_scope: &str,
) -> Result<(OutboxDispatchStats, i64, usize), Box<dyn std::error::Error>> {
    let export =
        commands::outbox::export_outbox_ndjson_for_consumer_floor(repo, consumer_tag, last_id)?;
    let start_id = export.start_id;
    if start_id >= export.head_id {
        return Ok((OutboxDispatchStats::default(), start_id, 0));
    }

    if export.ndjson.is_empty() {
        return Ok((OutboxDispatchStats::default(), start_id, 0));
    }

    let resolver = EngineResolver { store: &eng.store };
    let dispatch = if let Some(pp) = palace_path {
        let bank = mempalace_bank_from_viewer_scope(viewer_scope);
        let live = LiveMempalaceSink::open(pp, &bank)?;
        consume_outbox_ndjson_with_resolver_and_stats(&live, &resolver, &export.ndjson)?
    } else {
        consume_outbox_ndjson_with_resolver_and_stats(&CliMempalaceSink, &resolver, &export.ndjson)?
    };
    if dispatch.unresolved > 0 {
        return Err(format!(
            "consume-to-mempalace unresolved required events: unresolved={}",
            dispatch.unresolved
        )
        .into());
    }
    let acked = repo.mark_outbox_processed(export.head_id, consumer_tag)?;
    Ok((dispatch, start_id, acked))
}

pub(crate) fn current_governance_scan(
    eng: &LlmWikiEngine<NoopWikiHook>,
    schema: &DomainSchema,
    viewer: &Scope,
    now: OffsetDateTime,
) -> GovernanceScanReport {
    run_governance_scan(
        &eng.store,
        schema,
        GovernanceScanOptions {
            viewer_scope: Some(viewer),
            low_coverage_threshold: 2,
            generated_at: now,
            report_id: crate::governance::governance_report_prefix(now),
        },
    )
}

pub(crate) fn run_automation_governance_scan_job(
    eng: &LlmWikiEngine<NoopWikiHook>,
    schema: &DomainSchema,
    viewer: &Scope,
    wiki_root: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = OffsetDateTime::now_utc();
    let report = current_governance_scan(eng, schema, viewer, now);
    let dir = default_governance_report_dir(wiki_root);
    let files = crate::governance::write_scan_files(&report, &dir)?;
    print!("{}", crate::governance::render_scan_text(&report));
    println!("json_report_file={}", files.json_path.display());
    println!("markdown_report_file={}", files.markdown_path.display());
    Ok(())
}

pub(crate) fn build_automation_fixer_plan(
    cli: &Cli,
    scan_report: &GovernanceScanReport,
    now: OffsetDateTime,
    allow_web_search: bool,
) -> EvidenceFixerPlan {
    let (web_available, web_cross_verified_keys) = if allow_web_search {
        automation_cross_verify_duplicates(cli, scan_report, 5)
    } else {
        (false, BTreeSet::new())
    };
    build_evidence_fixer_plan(
        scan_report,
        EvidenceFixerPlanOptions {
            generated_at: now,
            plan_id: crate::governance::fixer_plan_prefix(now),
            web_available,
            web_cross_verified_keys,
            semantic_patch_proposals: Vec::new(),
        },
    )
}

pub(crate) fn automation_cross_verify_duplicates(
    cli: &Cli,
    scan_report: &GovernanceScanReport,
    max_web_checks: usize,
) -> (bool, BTreeSet<String>) {
    if max_web_checks == 0 {
        return (false, BTreeSet::new());
    }
    if scan_report
        .viewer_scope
        .as_deref()
        .is_some_and(|scope| scope.trim_start().starts_with("private:"))
    {
        eprintln!(
            "fixer-plan web verification skipped: private viewer_scope requires explicit CLI approval"
        );
        return (false, BTreeSet::new());
    }

    let app = match crate::llm::load_app_config(&cli.llm_config) {
        Ok(app) => app,
        Err(err) => {
            eprintln!("fixer-plan web verification skipped: {}", err);
            return (false, BTreeSet::new());
        }
    };

    let mut web_available = false;
    let mut verified = BTreeSet::new();
    for group in scan_report
        .duplicates
        .iter()
        .filter(|group| group.confidence != "exact")
        .take(max_web_checks)
    {
        let query = automation_duplicate_verification_query(group);
        match crate::web_search::run_search(&app, &[], &query) {
            Ok(run) => {
                if run.providers_succeeded.len() >= 2 {
                    web_available = true;
                }
                if run.cross_verified {
                    verified.insert(duplicate_web_verification_key(group));
                } else {
                    eprintln!(
                        "fixer-plan web verification not met: key={} providers={} domains={}",
                        duplicate_web_verification_key(group),
                        run.providers_succeeded.len(),
                        run.distinct_domains
                    );
                }
            }
            Err(err) => {
                eprintln!(
                    "fixer-plan web verification skipped for key={}: {}",
                    duplicate_web_verification_key(group),
                    err
                );
            }
        }
    }
    (web_available, verified)
}

pub(crate) fn automation_duplicate_verification_query(group: &GovernanceDuplicateGroup) -> String {
    let labels = group
        .members
        .iter()
        .filter_map(|member| member.label.as_deref())
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .collect::<Vec<_>>();
    if labels.is_empty() {
        format!("{} {}", group.kind, group.key)
    } else {
        format!(
            "verify whether these refer to the same object: {}",
            labels.join(" | ")
        )
    }
}

pub(crate) fn run_automation_fixer_plan_job(
    cli: &Cli,
    eng: &LlmWikiEngine<NoopWikiHook>,
    schema: &DomainSchema,
    viewer: &Scope,
    wiki_root: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = OffsetDateTime::now_utc();
    let scan_report = current_governance_scan(eng, schema, viewer, now);
    let plan = build_automation_fixer_plan(cli, &scan_report, now, true);
    let dir = default_fixer_report_dir(wiki_root);
    let files = crate::governance::write_fixer_plan_files(&plan, &dir)?;
    print!("{}", crate::governance::render_fixer_plan_text(&plan));
    println!("json_report_file={}", files.json_path.display());
    println!("markdown_report_file={}", files.markdown_path.display());
    Ok(())
}

pub(crate) fn latest_fixer_plan_path(
    dir: &Path,
) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    if !dir.exists() {
        return Ok(None);
    }
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.ends_with("-evidence-fixer-plan.json") {
            paths.push(path);
        }
    }
    paths.sort_by(|a, b| {
        let a_name = a.file_name().and_then(|name| name.to_str()).unwrap_or("");
        let b_name = b.file_name().and_then(|name| name.to_str()).unwrap_or("");
        a_name.cmp(b_name)
    });
    Ok(paths.pop())
}

pub(crate) fn run_automation_fixer_apply_job(
    cli: &Cli,
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    schema: &DomainSchema,
    viewer: &Scope,
    sync_wiki: bool,
    wiki_root: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = default_fixer_report_dir(wiki_root);
    let plan = match latest_fixer_plan_path(&dir)? {
        Some(path) => {
            println!("plan_file={}", path.display());
            serde_json::from_str::<EvidenceFixerPlan>(&std::fs::read_to_string(path)?)?
        }
        None => {
            let now = OffsetDateTime::now_utc();
            let scan_report = current_governance_scan(eng, schema, viewer, now);
            let plan = build_automation_fixer_plan(cli, &scan_report, now, false);
            let files = crate::governance::write_fixer_plan_files(&plan, &dir)?;
            println!("plan_file={}", files.json_path.display());
            plan
        }
    };
    let now = OffsetDateTime::now_utc();
    let report = apply_evidence_fixer_plan(
        eng,
        viewer,
        &plan,
        EvidenceFixerApplyOptions {
            generated_at: now,
            report_id: crate::governance::fixer_apply_report_prefix(now),
            policy: wiki_core::EvidenceFixerApplyPolicy::EvidenceAuto,
            apply: true,
        },
    );
    if report.summary.applied > 0 {
        eng.save_to_repo_and_flush_outbox_with_policy(repo, 128, 3)?;
        maybe_sync_projection(sync_wiki, wiki_root, eng)?;
    }
    let files = crate::governance::write_fixer_apply_files(&report, &dir)?;
    print!("{}", crate::governance::render_fixer_apply_text(&report));
    println!("json_report_file={}", files.json_path.display());
    println!("markdown_report_file={}", files.markdown_path.display());
    for path in files.tombstone_paths {
        println!("tombstone_file={}", path.display());
    }
    Ok(())
}

pub(crate) fn run_automation_synthesis_discover_job(
    eng: &LlmWikiEngine<NoopWikiHook>,
    schema: &DomainSchema,
    viewer: &Scope,
    wiki_root: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = OffsetDateTime::now_utc();
    let scan_report = current_governance_scan(eng, schema, viewer, now);
    let report = discover_synthesis_candidates(
        &scan_report,
        SynthesisDiscoveryOptions {
            generated_at: now,
            report_id: crate::governance::synthesis_discovery_report_prefix(now),
            max_single_double: 1,
            max_triple: 1,
            max_quad: 1,
        },
    );
    let dir = default_synthesis_report_dir(wiki_root);
    let files = crate::governance::write_synthesis_discovery_files(&report, &dir)?;
    print!(
        "{}",
        crate::governance::render_synthesis_discovery_text(&report)
    );
    println!("json_report_file={}", files.json_path.display());
    println!("markdown_report_file={}", files.markdown_path.display());
    Ok(())
}

pub(crate) fn run_automation_synthesis_run_job(
    cli: &Cli,
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    schema: &DomainSchema,
    viewer: &Scope,
    sync_wiki: bool,
    wiki_root: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = OffsetDateTime::now_utc();
    let scan_report = current_governance_scan(eng, schema, viewer, now);
    let discovery_report = discover_synthesis_candidates(
        &scan_report,
        SynthesisDiscoveryOptions {
            generated_at: now,
            report_id: crate::governance::synthesis_discovery_report_prefix(now),
            max_single_double: 1,
            max_triple: 1,
            max_quad: 1,
        },
    );
    let dir = default_synthesis_report_dir(wiki_root);
    let discovery_files =
        crate::governance::write_synthesis_discovery_files(&discovery_report, &dir)?;
    print!(
        "{}",
        crate::governance::render_synthesis_discovery_text(&discovery_report)
    );
    println!(
        "discovery_json_file={}",
        discovery_files.json_path.display()
    );
    println!(
        "discovery_markdown_file={}",
        discovery_files.markdown_path.display()
    );

    let mut reports = Vec::new();
    for candidate in &discovery_report.candidates {
        let report = run_research_synthesis_compose(
            eng,
            repo,
            viewer,
            sync_wiki,
            wiki_root,
            &cli.llm_config,
            &discovery_report,
            &candidate.candidate_id,
            ResearchSynthesisComposeInputs {
                web_evidence: None,
                draft_json: None,
                verifier_json: None,
                internal_only: false,
                allow_private_web_search: false,
                apply: true,
            },
        )?;
        let files = crate::research_synthesis::write_compose_files(&report, &dir)?;
        println!("json_report_file={}", files.json_path.display());
        println!("markdown_report_file={}", files.markdown_path.display());
        reports.push(report);
    }
    println!("synthesis run: reports={}", reports.len());
    Ok(())
}

pub(crate) fn dispatch_automation_job(
    job: AutomationJob,
    heartbeat: &AutomationHeartbeat<'_>,
    cli: &Cli,
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    viewer: &Scope,
    sync_wiki: bool,
    wiki_root: Option<&std::path::Path>,
    schema: &DomainSchema,
) -> Result<(), Box<dyn std::error::Error>> {
    heartbeat.tick();
    match job {
        AutomationJob::NotionSync => run_notion_sync_job(
            eng,
            repo,
            viewer,
            if sync_wiki { wiki_root } else { None },
            false,
            0,
            false,
        ),
        AutomationJob::BatchIngest => {
            let vault = cli
                .wiki_dir
                .clone()
                .unwrap_or_else(crate::wiki_compiler::default_vault_path);
            crate::wiki_compiler::batch_ingest_cmd(
                eng,
                repo,
                &cli.llm_config,
                cli.vectors,
                schema,
                heartbeat,
                crate::wiki_compiler::BatchIngestOptions {
                    vault: &vault,
                    limit: None,
                    dry_run: false,
                    delay_secs: 1,
                    sync_wiki,
                    wiki_root,
                    scope: None,
                    origin: None,
                    source_path: None,
                },
            )
        }
        AutomationJob::GovernanceScan => {
            run_automation_governance_scan_job(eng, schema, viewer, wiki_root)
        }
        AutomationJob::FixerPlan => {
            run_automation_fixer_plan_job(cli, eng, schema, viewer, wiki_root)
        }
        AutomationJob::FixerApply => {
            run_automation_fixer_apply_job(cli, eng, repo, schema, viewer, sync_wiki, wiki_root)
        }
        AutomationJob::Lint => run_lint_job(eng, repo, viewer, sync_wiki, wiki_root),
        AutomationJob::Maintenance => run_maintenance_job(eng, repo, viewer, sync_wiki, wiki_root),
        AutomationJob::ConsumeToMempalace => run_consume_to_mempalace_job(
            eng,
            repo,
            DEFAULT_MEMPALACE_CONSUMER_TAG,
            0,
            cli.palace.as_deref(),
            &cli.viewer_scope,
        )
        .map(|_| ()),
        AutomationJob::LlmSmoke => {
            let cfg = crate::llm::load_llm_config(&cli.llm_config)?;
            let out = crate::llm::smoke_chat_completion(&cfg, "Say 'ok' only.")?;
            println!("{out}");
            Ok(())
        }
        AutomationJob::VaultReports => {
            crate::run_scheduled_vault_reports_job(eng, repo, viewer, schema, wiki_root)
        }
        AutomationJob::SynthesisDiscover => {
            run_automation_synthesis_discover_job(eng, schema, viewer, wiki_root)
        }
        AutomationJob::SynthesisRun => {
            run_automation_synthesis_run_job(cli, eng, repo, schema, viewer, sync_wiki, wiki_root)
        }
    }
}

pub(crate) fn run_single_automation_job<W: Write>(
    out: &mut W,
    job: AutomationJob,
    cli: &Cli,
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    viewer: &Scope,
    sync_wiki: bool,
    wiki_root: Option<&std::path::Path>,
    schema: &DomainSchema,
) -> Result<(), Box<dyn std::error::Error>> {
    let spec = automation_job_spec(job);
    writeln!(
        out,
        "automation: running {} requires_network={} daily={}",
        automation_job_name(spec.job),
        if spec.requires_network { "yes" } else { "no" },
        if spec.in_daily { "yes" } else { "no" }
    )?;
    run_automation_job(repo, job, |hb| {
        dispatch_automation_job(
            job, hb, cli, eng, repo, viewer, sync_wiki, wiki_root, schema,
        )
    })?;
    let latest = latest_automation_run_or_error(repo, job)?;
    writeln!(
        out,
        "automation: finished {} {}",
        automation_job_name(job),
        format_automation_record(&latest)
    )?;
    Ok(())
}

pub(crate) fn apply_notion_sync_tag_policy(schema: &mut DomainSchema, policy: NotionSyncTagPolicy) {
    match policy {
        NotionSyncTagPolicy::Strict => {}
        NotionSyncTagPolicy::TrustedSource | NotionSyncTagPolicy::Bootstrap => {
            schema.tag_config.max_new_tags_per_ingest = u32::MAX;
            schema.tag_config.deprecated_tags.clear();
            eprintln!(
                "notion-sync: tag_policy={:?} max_new_tags_per_ingest=unlimited deprecated_tags=allow",
                policy
            );
        }
    }
}

pub(crate) fn automation_notion_refresh_existing() -> bool {
    true
}

pub(crate) fn run_notion_sync_cmd(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    viewer: &Scope,
    source_projection_vault: Option<&std::path::Path>,
    db_id: NotionDbTarget,
    since: Option<&str>,
    limit: Option<usize>,
    dry_run: bool,
    request_delay_ms: u64,
    writeback_notion: bool,
    refresh_existing: bool,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::notion_client::NotionApiClient;
    use crate::notion_sync::NotionSyncRunner;
    use crate::notion_writeback::{HttpNotionWriteBack, NoopWriteBack};
    use time::format_description::well_known::Rfc3339;

    let since_time = since
        .map(|s| OffsetDateTime::parse(s, &Rfc3339))
        .transpose()
        .map_err(|e| format!("invalid --since value: {e}"))?;

    let mut client = NotionApiClient::from_env()?.with_request_delay_ms(request_delay_ms);

    let writeback: Box<dyn crate::notion_writeback::NotionWriteBackClient> = if writeback_notion {
        let token = std::env::var("NOTION_TOKEN").unwrap_or_default();
        Box::new(HttpNotionWriteBack::new(token))
    } else {
        Box::new(NoopWriteBack)
    };

    let dbs: Vec<(&str, &str)> = match db_id {
        NotionDbTarget::XBookmark => vec![NOTION_DB_X_BOOKMARK],
        NotionDbTarget::Wechat => vec![NOTION_DB_WECHAT],
        NotionDbTarget::All => vec![NOTION_DB_X_BOOKMARK, NOTION_DB_WECHAT],
    };

    for (slug, notion_id) in dbs {
        let mut runner = NotionSyncRunner::new(&mut client, repo, eng, viewer.clone(), verbose);
        let result = runner.run_sync(
            slug,
            notion_id,
            since_time,
            limit,
            dry_run,
            refresh_existing,
            writeback.as_ref(),
        )?;
        println!(
            "notion-sync db={} fetched={} new={} refreshed={} skipped={} errors={} duration={:.1}s{}",
            result.db_id,
            result.fetched,
            result.new,
            result.refreshed,
            result.skipped,
            result.errors,
            result.duration_secs,
            if dry_run { " [dry-run]" } else { "" }
        );
    }

    if !dry_run {
        if let Some(vault) = source_projection_vault {
            let sources: Vec<_> = eng.store.sources.values().cloned().collect();
            let mode = crate::notion_source_projection::ProjectionMode::Apply;
            let report = if refresh_existing {
                crate::notion_source_projection::project_notion_sources_to_vault_with_options(
                    &sources,
                    vault,
                    crate::notion_source_projection::ProjectionOptions {
                        mode,
                        refresh_existing,
                    },
                )?
            } else {
                crate::notion_source_projection::project_notion_sources_to_vault(
                    &sources, vault, mode,
                )?
            };
            println!("{report}");
        }
    }

    Ok(())
}

pub(crate) fn run_notion_sync_job(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    viewer: &Scope,
    source_projection_vault: Option<&std::path::Path>,
    dry_run: bool,
    request_delay_ms: u64,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let request_delay_ms = if request_delay_ms > 0 {
        request_delay_ms
    } else {
        env_or("NOTION_SYNC_DELAY_MS", 350)
    };
    apply_notion_sync_tag_policy(&mut eng.schema, NotionSyncTagPolicy::TrustedSource);
    run_notion_sync_cmd(
        eng,
        repo,
        viewer,
        source_projection_vault,
        NotionDbTarget::All,
        None,
        None,
        dry_run,
        request_delay_ms,
        false,
        automation_notion_refresh_existing(),
        verbose,
    )
}

pub(crate) fn run_daily_automation(
    cli: &Cli,
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    viewer: &Scope,
    sync_wiki: bool,
    wiki_root: Option<&std::path::Path>,
    schema: &DomainSchema,
) -> Result<(), Box<dyn std::error::Error>> {
    let jobs = automation_run_daily_jobs();
    let mut stdout = std::io::stdout().lock();
    run_automation_plan(&jobs, false, &mut stdout, |job| {
        run_automation_job(repo, job, |hb| {
            dispatch_automation_job(
                job, hb, cli, eng, repo, viewer, sync_wiki, wiki_root, schema,
            )
        })
    })
}

pub(crate) fn query_to_page(
    title: &str,
    query: &str,
    ranked: &[(String, f64)],
    scope: Scope,
    entry_type: Option<EntryType>,
    schema: &DomainSchema,
) -> WikiPage {
    let et = entry_type.unwrap_or(EntryType::Qa);

    // 拼回答内容（ranked results 列表）
    let mut answer = String::new();
    for (doc, score) in ranked.iter().take(20) {
        answer.push_str(&format!("- `{doc}` score={score:.6}\n"));
    }

    let status = initial_status_for(Some(&et), schema);

    PageContract::new(title, et)
        .with_section("问题", query)
        .with_section("回答", answer.trim_end())
        .with_source("query")
        .into_page(scope, status)
}

pub(crate) fn read_graph_extras_lines(
    path: &PathBuf,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let s = std::fs::read_to_string(path)?;
    Ok(s.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect())
}
