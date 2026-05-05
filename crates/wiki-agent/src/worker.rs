use crate::config::AgentConfig;
use crate::evidence::EvidencePack;
use crate::planner::{self, WebMode};
use crate::roles::{WorkerAccess, WorkerRole};
use crate::task::AgentTask;
use crate::tool_backend::ToolBackendKind;
use crate::worker_tools::{acquire_writer_lease, EngineContext};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use time::OffsetDateTime;
use wiki_core::{EvidenceFixerApplyPolicy, QueryContext, SemanticPatchProposal};
use wiki_kernel::{
    apply_evidence_fixer_plan, build_evidence_fixer_plan, collect_basic_lint_findings,
    discover_synthesis_candidates, run_governance_scan, write_projection,
    EvidenceFixerApplyOptions, EvidenceFixerPlanOptions, GovernanceScanOptions,
    SynthesisDiscoveryOptions,
};
use wiki_storage::SqliteSearchPorts;

#[derive(Clone, Debug)]
pub struct WorkerRuntimeOptions {
    pub backend: ToolBackendKind,
    pub web: WebMode,
    pub web_providers: Vec<String>,
    pub allow_private_web_search: bool,
    pub wiki_cli: Option<PathBuf>,
    pub web_evidence_json: Option<PathBuf>,
}

pub struct WorkerRuntime<'a> {
    config: &'a AgentConfig,
    options: WorkerRuntimeOptions,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    Completed,
    Blocked,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCallSummary {
    pub name: String,
    pub backend: String,
    pub ok: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerReport {
    pub task_id: String,
    pub role: WorkerRole,
    pub role_name: String,
    pub access: WorkerAccess,
    pub status: WorkerStatus,
    pub input: String,
    pub tool_calls: Vec<ToolCallSummary>,
    pub artifacts: Vec<String>,
    pub blockers: Vec<String>,
    pub output: Value,
}

impl WorkerReport {
    fn completed(task: &AgentTask, output: Value, tool_calls: Vec<ToolCallSummary>) -> Self {
        Self::new(
            task,
            WorkerStatus::Completed,
            output,
            tool_calls,
            Vec::new(),
        )
    }

    fn blocked(task: &AgentTask, reason: impl Into<String>) -> Self {
        Self::new(
            task,
            WorkerStatus::Blocked,
            json!({}),
            Vec::new(),
            vec![reason.into()],
        )
    }

    fn failed(task: &AgentTask, error: impl Into<String>) -> Self {
        Self::new(
            task,
            WorkerStatus::Failed,
            json!({}),
            Vec::new(),
            vec![error.into()],
        )
    }

    fn new(
        task: &AgentTask,
        status: WorkerStatus,
        output: Value,
        tool_calls: Vec<ToolCallSummary>,
        blockers: Vec<String>,
    ) -> Self {
        Self {
            task_id: task.task_id.clone(),
            role: task.role,
            role_name: task.role.agent_name().to_string(),
            access: task.role.access(task.apply),
            status,
            input: task.goal.clone(),
            tool_calls,
            artifacts: Vec::new(),
            blockers,
            output,
        }
    }

    pub fn render_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "role={} status={:?} access={:?}\n",
            self.role_name, self.status, self.access
        ));
        for call in &self.tool_calls {
            out.push_str(&format!(
                "tool={} backend={} ok={}\n",
                call.name, call.backend, call.ok
            ));
        }
        for blocker in &self.blockers {
            out.push_str(&format!("blocker={blocker}\n"));
        }
        if self.output != json!({}) {
            out.push_str(&format!("output={}\n", self.output));
        }
        out
    }
}

impl<'a> WorkerRuntime<'a> {
    pub fn new(config: &'a AgentConfig, options: WorkerRuntimeOptions) -> Self {
        Self { config, options }
    }

    pub fn execute(&self, task: &AgentTask) -> WorkerReport {
        match self.execute_inner(task) {
            Ok(report) => report,
            Err(err) => WorkerReport::failed(task, err.to_string()),
        }
    }

    fn execute_inner(&self, task: &AgentTask) -> Result<WorkerReport, Box<dyn std::error::Error>> {
        match self.options.backend {
            ToolBackendKind::Native | ToolBackendKind::Auto => self.execute_native(task),
            ToolBackendKind::McpChild => self.execute_mcp_child(task),
        }
    }

    fn execute_mcp_child(
        &self,
        task: &AgentTask,
    ) -> Result<WorkerReport, Box<dyn std::error::Error>> {
        let Some(tool) = task.role.mcp_tool_name() else {
            return Ok(WorkerReport::blocked(
                task,
                format!(
                    "{} requires native Rust core; mcp-child exposes only MCP tools",
                    task.role.agent_name()
                ),
            ));
        };
        let args = tool_args_for_role(task);
        let value = crate::mcp_fallback::call_child_tool(
            self.config,
            self.options.wiki_cli.as_deref(),
            tool,
            args,
        )?;
        Ok(WorkerReport::completed(
            task,
            value,
            vec![ToolCallSummary {
                name: tool.to_string(),
                backend: "mcp-child".to_string(),
                ok: true,
            }],
        ))
    }

    fn execute_native(&self, task: &AgentTask) -> Result<WorkerReport, Box<dyn std::error::Error>> {
        match task.role {
            WorkerRole::Lint => self.native_lint(task),
            WorkerRole::Search => self.native_search(task),
            WorkerRole::Governance => self.native_governance(task),
            WorkerRole::Fixer => self.native_fixer(task),
            WorkerRole::Synthesis => self.native_synthesis(task),
            WorkerRole::MemoryCurator => self.native_memory_curator(task),
        }
    }

    fn native_lint(&self, task: &AgentTask) -> Result<WorkerReport, Box<dyn std::error::Error>> {
        let ctx = EngineContext::open(self.config)?;
        let findings = collect_basic_lint_findings(&ctx.schema, &ctx.eng.store, Some(&ctx.viewer));
        let value = json!({
            "findings": findings,
            "findings_count": findings.len(),
        });
        Ok(WorkerReport::completed(
            task,
            value,
            vec![native_call("collect_basic_lint_findings")],
        ))
    }

    fn native_search(&self, task: &AgentTask) -> Result<WorkerReport, Box<dyn std::error::Error>> {
        let ctx = EngineContext::open(self.config)?;
        let ports = SqliteSearchPorts::open(&ctx.repo, Some(ctx.viewer.clone()))?;
        let query_ctx = QueryContext::new(&task.goal)
            .with_per_stream_limit(5)
            .with_viewer_scope(ctx.viewer.clone());
        let ranked = ctx.eng.query_ranked_with_ports(
            &query_ctx,
            OffsetDateTime::now_utc(),
            &ports,
            None,
            None,
        );
        let mut output = json!({
            "internal": {
                "results": ranked.iter().take(20).map(|(id, score)| json!({
                    "doc_id": id,
                    "score": score,
                })).collect::<Vec<_>>()
            }
        });
        let mut calls = vec![native_call("query_ranked_with_ports")];

        if planner::should_use_web(&task.goal, self.options.web)
            && !planner::private_scope_blocks_web(
                &self.config.viewer_scope,
                self.options.allow_private_web_search,
            )
        {
            let web = crate::web_tool::run_web_search(
                self.config,
                &task.goal,
                &self.options.web_providers,
                self.options.web_evidence_json.as_deref(),
            )?;
            if !web.cross_verified {
                return Ok(WorkerReport::blocked(
                    task,
                    "web search evidence is not cross-verified",
                ));
            }
            output["web"] = serde_json::to_value(web)?;
            calls.push(ToolCallSummary {
                name: "web_search".to_string(),
                backend: "wiki-ai".to_string(),
                ok: true,
            });
        }

        Ok(WorkerReport::completed(task, output, calls))
    }

    fn native_governance(
        &self,
        task: &AgentTask,
    ) -> Result<WorkerReport, Box<dyn std::error::Error>> {
        let ctx = EngineContext::open(self.config)?;
        let scan = governance_scan(&ctx, &format!("{}-governance", task.task_id));
        let output = json!({
            "report_id": scan.report_id,
            "summary": scan.summary,
        });
        Ok(WorkerReport::completed(
            task,
            output,
            vec![native_call("run_governance_scan")],
        ))
    }

    fn native_fixer(&self, task: &AgentTask) -> Result<WorkerReport, Box<dyn std::error::Error>> {
        if task.apply {
            let _lease = match acquire_writer_lease(self.config, "fixer-agent") {
                Ok(lease) => lease,
                Err(err) => return Ok(WorkerReport::blocked(task, err.to_string())),
            };
            let mut ctx = EngineContext::open(self.config)?;
            let scan = governance_scan(&ctx, &format!("{}-fixer-scan", task.task_id));
            let plan = fixer_plan(&scan, &format!("{}-fixer-plan", task.task_id));
            let apply = apply_evidence_fixer_plan(
                &mut ctx.eng,
                &ctx.viewer,
                &plan,
                EvidenceFixerApplyOptions {
                    generated_at: OffsetDateTime::now_utc(),
                    report_id: format!("{}-fixer-apply", task.task_id),
                    policy: EvidenceFixerApplyPolicy::EvidenceAuto,
                    apply: true,
                },
            );
            ctx.eng.save_to_repo_and_flush_outbox(&ctx.repo)?;
            if let Some(wiki_dir) = &self.config.wiki_dir {
                write_projection(wiki_dir, &ctx.eng.store, &ctx.eng.audits)?;
            }
            let output = json!({
                "plan_summary": plan.summary,
                "apply_summary": apply.summary,
            });
            return Ok(WorkerReport::completed(
                task,
                output,
                vec![
                    native_call("run_governance_scan"),
                    native_call("build_evidence_fixer_plan"),
                    native_call("apply_evidence_fixer_plan"),
                ],
            ));
        }

        let ctx = EngineContext::open(self.config)?;
        let scan = governance_scan(&ctx, &format!("{}-fixer-scan", task.task_id));
        let plan = fixer_plan(&scan, &format!("{}-fixer-plan", task.task_id));
        let output = json!({
            "plan_id": plan.plan_id,
            "summary": plan.summary,
        });
        Ok(WorkerReport::completed(
            task,
            output,
            vec![
                native_call("run_governance_scan"),
                native_call("build_evidence_fixer_plan"),
            ],
        ))
    }

    fn native_synthesis(
        &self,
        task: &AgentTask,
    ) -> Result<WorkerReport, Box<dyn std::error::Error>> {
        let ctx = EngineContext::open(self.config)?;
        let scan = governance_scan(&ctx, &format!("{}-synthesis-scan", task.task_id));
        let report = discover_synthesis_candidates(
            &scan,
            SynthesisDiscoveryOptions {
                generated_at: OffsetDateTime::now_utc(),
                report_id: format!("{}-synthesis-discovery", task.task_id),
                max_single_double: 1,
                max_triple: 1,
                max_quad: 1,
            },
        );
        let output = json!({
            "report_id": report.report_id,
            "summary": report.summary,
            "candidates": report.candidates,
        });
        Ok(WorkerReport::completed(
            task,
            output,
            vec![
                native_call("run_governance_scan"),
                native_call("discover_synthesis_candidates"),
            ],
        ))
    }

    fn native_memory_curator(
        &self,
        task: &AgentTask,
    ) -> Result<WorkerReport, Box<dyn std::error::Error>> {
        let store = crate::session_store::SessionStore::open(self.config.session_db_path())?;
        let Some(session) = store.list_sessions()?.into_iter().next() else {
            return Ok(WorkerReport::blocked(task, "no sessions available"));
        };
        let report = crate::memory::curate_session(self.config, &store, &session.id, task.apply)?;
        let output = serde_json::to_value(report)?;
        Ok(WorkerReport::completed(
            task,
            output,
            vec![native_call("memory_curate_session")],
        ))
    }
}

fn native_call(name: &str) -> ToolCallSummary {
    ToolCallSummary {
        name: name.to_string(),
        backend: "native".to_string(),
        ok: true,
    }
}

fn tool_args_for_role(task: &AgentTask) -> Value {
    match task.role {
        WorkerRole::Search => json!({"query": task.goal, "per_stream_limit": 5}),
        _ => json!({}),
    }
}

fn governance_scan(ctx: &EngineContext, report_id: &str) -> wiki_core::GovernanceScanReport {
    run_governance_scan(
        &ctx.eng.store,
        &ctx.schema,
        GovernanceScanOptions {
            viewer_scope: Some(&ctx.viewer),
            low_coverage_threshold: 2,
            generated_at: OffsetDateTime::now_utc(),
            report_id: report_id.to_string(),
        },
    )
}

fn fixer_plan(
    scan: &wiki_core::GovernanceScanReport,
    plan_id: &str,
) -> wiki_core::EvidenceFixerPlan {
    build_evidence_fixer_plan(
        scan,
        EvidenceFixerPlanOptions {
            generated_at: OffsetDateTime::now_utc(),
            plan_id: plan_id.to_string(),
            web_available: false,
            web_cross_verified_keys: Default::default(),
            semantic_patch_proposals: Vec::<SemanticPatchProposal>::new(),
        },
    )
}

#[allow(dead_code)]
fn evidence_pack_for_output(internal: Vec<crate::evidence::InternalEvidence>) -> EvidencePack {
    EvidencePack::new(internal)
}
