use serde::Serialize;
use time::OffsetDateTime;
use wiki_core::{
    StrategyExecutionActionKind, StrategyExecutionDryRunStatus, StrategyExecutionPlan,
    StrategyExecutionPolicy, StrategyReport, StrategySeverity, WikiEvent, WikiMetricsReport,
};

use crate::automation::format_automation_time;
use crate::cli_utils::{entry_status_name, entry_type_name, format_optional_i64};

pub(crate) fn render_metrics_text(report: &WikiMetricsReport) -> String {
    let generated_at = report
        .generated_at
        .map(format_automation_time)
        .unwrap_or_else(|| "unknown".to_string());
    let consumer_tag = report
        .outbox
        .consumer_tag
        .as_deref()
        .unwrap_or("none")
        .to_string();
    let page_status = report
        .lifecycle
        .page_status
        .iter()
        .map(|item| format!("{}={}", entry_status_name(item.status), item.count))
        .collect::<Vec<_>>()
        .join(" ");
    let entry_type = report
        .lifecycle
        .entry_type
        .iter()
        .map(|item| format!("{}={}", entry_type_name(&item.entry_type), item.count))
        .collect::<Vec<_>>()
        .join(" ");

    format!(
        concat!(
            "metrics report: generated_at={}\n",
            "content: sources={} pages={} claims={} entities={} relations={}\n",
            "lint: total_findings={} info={} warn={} error={}\n",
            "gaps: total_findings={} low={} medium={} high={}\n",
            "outbox: head_id={} total_events={} unprocessed_events={} consumer_tag={} acked_up_to_id={} backlog_events={}\n",
            "lifecycle: stale_claims={} page_status=[{}] entry_type=[{}]\n",
        ),
        generated_at,
        report.content.sources,
        report.content.pages,
        report.content.claims,
        report.content.entities,
        report.content.relations,
        report.lint.total_findings,
        report.lint.severity.info,
        report.lint.severity.warn,
        report.lint.severity.error,
        report.gaps.total_findings,
        report.gaps.severity.low,
        report.gaps.severity.medium,
        report.gaps.severity.high,
        format_optional_i64(report.outbox.head_id),
        report.outbox.total_events,
        report.outbox.unprocessed_events,
        consumer_tag,
        format_optional_i64(report.outbox.acked_up_to_id),
        report.outbox.backlog_events,
        report.lifecycle.stale_claims,
        page_status,
        entry_type,
    )
}

pub(crate) fn render_metrics_markdown(report: &WikiMetricsReport) -> String {
    let generated_at = report
        .generated_at
        .map(format_automation_time)
        .unwrap_or_else(|| "unknown".to_string());
    let page_status_rows = report
        .lifecycle
        .page_status
        .iter()
        .map(|item| format!("- {}: {}", entry_status_name(item.status), item.count))
        .collect::<Vec<_>>()
        .join("\n");
    let entry_type_rows = report
        .lifecycle
        .entry_type
        .iter()
        .map(|item| format!("- {}: {}", entry_type_name(&item.entry_type), item.count))
        .collect::<Vec<_>>()
        .join("\n");
    let consumer_tag = report.outbox.consumer_tag.as_deref().unwrap_or("none");

    format!(
        concat!(
            "# Wiki Metrics Report\n\n",
            "Generated at: {}\n\n",
            "## Content\n\n",
            "- Sources: {}\n",
            "- Pages: {}\n",
            "- Claims: {}\n",
            "- Entities: {}\n",
            "- Relations: {}\n\n",
            "## Lint\n\n",
            "- Total findings: {}\n",
            "- Info: {}\n",
            "- Warn: {}\n",
            "- Error: {}\n\n",
            "## Gaps\n\n",
            "- Total findings: {}\n",
            "- Low: {}\n",
            "- Medium: {}\n",
            "- High: {}\n\n",
            "## Outbox\n\n",
            "- Head id: {}\n",
            "- Total events: {}\n",
            "- Unprocessed events: {}\n",
            "- Consumer tag: {}\n",
            "- Acked up to id: {}\n",
            "- Backlog events: {}\n\n",
            "## Lifecycle\n\n",
            "- Stale claims: {}\n\n",
            "Page status:\n{}\n\n",
            "Entry type:\n{}\n",
        ),
        generated_at,
        report.content.sources,
        report.content.pages,
        report.content.claims,
        report.content.entities,
        report.content.relations,
        report.lint.total_findings,
        report.lint.severity.info,
        report.lint.severity.warn,
        report.lint.severity.error,
        report.gaps.total_findings,
        report.gaps.severity.low,
        report.gaps.severity.medium,
        report.gaps.severity.high,
        format_optional_i64(report.outbox.head_id),
        report.outbox.total_events,
        report.outbox.unprocessed_events,
        consumer_tag,
        format_optional_i64(report.outbox.acked_up_to_id),
        report.outbox.backlog_events,
        report.lifecycle.stale_claims,
        page_status_rows,
        entry_type_rows,
    )
}

pub(crate) fn strategy_severity_name(severity: StrategySeverity) -> &'static str {
    match severity {
        StrategySeverity::Low => "low",
        StrategySeverity::Medium => "medium",
        StrategySeverity::High => "high",
    }
}

pub(crate) fn strategy_execution_policy_name(policy: StrategyExecutionPolicy) -> &'static str {
    match policy {
        StrategyExecutionPolicy::AutoSafe => "auto_safe",
        StrategyExecutionPolicy::AgentReview => "agent_review",
        StrategyExecutionPolicy::HumanRequired => "human_required",
    }
}

pub(crate) fn strategy_execution_action_kind_name(kind: StrategyExecutionActionKind) -> &'static str {
    match kind {
        StrategyExecutionActionKind::FixAutoSafe => "fix_auto_safe",
        StrategyExecutionActionKind::AgentReview => "agent_review",
        StrategyExecutionActionKind::HumanRequired => "human_required",
        StrategyExecutionActionKind::Unsupported => "unsupported",
    }
}

pub(crate) fn strategy_dry_run_status_name(status: StrategyExecutionDryRunStatus) -> &'static str {
    match status {
        StrategyExecutionDryRunStatus::WouldApply => "would_apply",
        StrategyExecutionDryRunStatus::Blocked => "blocked",
    }
}

pub(crate) fn render_strategy_report_text(report: &StrategyReport) -> String {
    let generated_at = report
        .generated_at
        .map(format_automation_time)
        .unwrap_or_else(|| "unknown".to_string());
    let viewer_scope = report.viewer_scope.as_deref().unwrap_or("none");
    let mut out = format!(
        "strategy suggestions: report_id={} generated_at={} viewer_scope={} suggestions={}\n",
        report.report_id,
        generated_at,
        viewer_scope,
        report.suggestions.len()
    );
    for suggestion in &report.suggestions {
        out.push_str(&format!(
            "- id={} code={} severity={} subject={} execution_policy={}\n  reason={}\n",
            suggestion.suggestion_id,
            suggestion.code,
            strategy_severity_name(suggestion.severity),
            suggestion.subject.as_deref().unwrap_or("none"),
            strategy_execution_policy_name(suggestion.execution_policy),
            suggestion.reason
        ));
        if let Some(command) = &suggestion.suggested_command {
            out.push_str(&format!("  suggested_command={command}\n"));
        }
    }
    out
}

pub(crate) fn render_strategy_execution_plan_text(plan: &StrategyExecutionPlan) -> String {
    let generated_at = plan
        .generated_at
        .map(format_automation_time)
        .unwrap_or_else(|| "unknown".to_string());
    let viewer_scope = plan.viewer_scope.as_deref().unwrap_or("none");
    let would_apply = plan
        .actions
        .iter()
        .filter(|action| action.dry_run_status == StrategyExecutionDryRunStatus::WouldApply)
        .count();
    let blocked = plan.actions.len().saturating_sub(would_apply);
    let mut out = format!(
        "executor dry-run plan: plan_id={} source_report_id={} generated_at={} viewer_scope={} actions={} would_apply={} blocked={}\n",
        plan.plan_id,
        plan.source_report_id,
        generated_at,
        viewer_scope,
        plan.actions.len(),
        would_apply,
        blocked
    );
    for action in &plan.actions {
        out.push_str(&format!(
            "- id={} suggestion_id={} code={} action_kind={} dry_run_status={}\n  reason={}\n",
            action.action_id,
            action.suggestion_id,
            action.code,
            strategy_execution_action_kind_name(action.action_kind),
            strategy_dry_run_status_name(action.dry_run_status),
            action.reason
        ));
        if let Some(command) = &action.command_preview {
            out.push_str(&format!("  command_preview={command}\n"));
        }
    }
    out
}

pub(crate) fn render_strategy_report_markdown(report: &StrategyReport, sibling_json: &str) -> String {
    let generated_at = report
        .generated_at
        .map(format_automation_time)
        .unwrap_or_else(|| "unknown".to_string());
    let viewer_scope = report.viewer_scope.as_deref().unwrap_or("none");
    let mut out = format!(
        concat!(
            "# M12 Strategy Suggestions\n\n",
            "- report_id: {}\n",
            "- generated_at: {}\n",
            "- viewer_scope: {}\n",
            "- suggestion_count: {}\n",
            "- source_of_truth: {}\n\n",
            "> Sibling JSON `{}` is the source of truth. This Markdown is rendered from the same StrategyReport.\n\n",
            "## Suggestions\n\n",
        ),
        report.report_id,
        generated_at,
        viewer_scope,
        report.suggestions.len(),
        sibling_json,
        sibling_json
    );
    if report.suggestions.is_empty() {
        out.push_str("No suggestions.\n");
        return out;
    }
    for suggestion in &report.suggestions {
        out.push_str(&format!(
            concat!(
                "### {}\n\n",
                "- code: {}\n",
                "- severity: {}\n",
                "- subject: {}\n",
                "- execution_policy: {}\n",
                "- reason: {}\n",
            ),
            suggestion.suggestion_id,
            suggestion.code,
            strategy_severity_name(suggestion.severity),
            suggestion.subject.as_deref().unwrap_or("none"),
            strategy_execution_policy_name(suggestion.execution_policy),
            suggestion.reason
        ));
        if let Some(command) = &suggestion.suggested_command {
            out.push_str(&format!("- suggested_command: `{command}`\n"));
        }
        out.push('\n');
    }
    out
}

pub(crate) fn render_strategy_execution_plan_markdown(
    plan: &StrategyExecutionPlan,
    sibling_json: &str,
    source_report_json: &str,
) -> String {
    let generated_at = plan
        .generated_at
        .map(format_automation_time)
        .unwrap_or_else(|| "unknown".to_string());
    let viewer_scope = plan.viewer_scope.as_deref().unwrap_or("none");
    let would_apply = plan
        .actions
        .iter()
        .filter(|action| action.dry_run_status == StrategyExecutionDryRunStatus::WouldApply)
        .count();
    let blocked = plan.actions.len().saturating_sub(would_apply);
    let mut out = format!(
        concat!(
            "# M12 Executor Dry-Run Plan\n\n",
            "- plan_id: {}\n",
            "- source_report_id: {}\n",
            "- generated_at: {}\n",
            "- viewer_scope: {}\n",
            "- mode: dry_run\n",
            "- action_count: {}\n",
            "- would_apply: {}\n",
            "- blocked: {}\n",
            "- source_of_truth: {}\n",
            "- source_report: {}\n\n",
            "> Sibling JSON `{}` is the source of truth. This plan is derived from `{}` and does not execute writes.\n\n",
            "## Actions\n\n",
        ),
        plan.plan_id,
        plan.source_report_id,
        generated_at,
        viewer_scope,
        plan.actions.len(),
        would_apply,
        blocked,
        sibling_json,
        source_report_json,
        sibling_json,
        source_report_json
    );
    if plan.actions.is_empty() {
        out.push_str("No actions.\n");
        return out;
    }
    for action in &plan.actions {
        out.push_str(&format!(
            concat!(
                "### {}\n\n",
                "- suggestion_id: {}\n",
                "- code: {}\n",
                "- execution_policy: {}\n",
                "- action_kind: {}\n",
                "- dry_run_status: {}\n",
                "- subject: {}\n",
                "- reason: {}\n",
            ),
            action.action_id,
            action.suggestion_id,
            action.code,
            strategy_execution_policy_name(action.execution_policy),
            strategy_execution_action_kind_name(action.action_kind),
            strategy_dry_run_status_name(action.dry_run_status),
            action.subject.as_deref().unwrap_or("none"),
            action.reason
        ));
        if let Some(command) = &action.command_preview {
            out.push_str(&format!("- command_preview: `{command}`\n"));
        }
        out.push('\n');
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StrategyExecutorRunMode {
    Preflight,
    Apply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StrategyExecutorActionStatus {
    WouldApply,
    Applied,
    Blocked,
    Skipped,
}

#[derive(Debug, Serialize)]
pub(crate) struct StrategyExecutorApplySummary {
    pub total: usize,
    pub would_apply: usize,
    pub applied: usize,
    pub blocked: usize,
    pub skipped: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct StrategyExecutorApplyActionReport {
    pub action_id: String,
    pub suggestion_id: String,
    pub code: String,
    pub subject: Option<String>,
    pub action_kind: StrategyExecutionActionKind,
    pub status: StrategyExecutorActionStatus,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct StrategyExecutorApplyReport {
    pub report_id: String,
    pub plan_id: String,
    pub source_report_id: String,
    pub generated_at: OffsetDateTime,
    pub mode: StrategyExecutorRunMode,
    pub apply_requested: bool,
    pub allowlist: Vec<String>,
    pub summary: StrategyExecutorApplySummary,
    pub actions: Vec<StrategyExecutorApplyActionReport>,
}

pub(crate) fn strategy_executor_report_prefix(generated_at: OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}{:02}{:02}.{:09}Z-m12-executor",
        generated_at.year(),
        generated_at.month() as u8,
        generated_at.day(),
        generated_at.hour(),
        generated_at.minute(),
        generated_at.second(),
        generated_at.nanosecond()
    )
}

pub(crate) fn strategy_executor_action_status_name(status: StrategyExecutorActionStatus) -> &'static str {
    match status {
        StrategyExecutorActionStatus::WouldApply => "would_apply",
        StrategyExecutorActionStatus::Applied => "applied",
        StrategyExecutorActionStatus::Blocked => "blocked",
        StrategyExecutorActionStatus::Skipped => "skipped",
    }
}

pub(crate) fn render_strategy_executor_apply_report_text(report: &StrategyExecutorApplyReport) -> String {
    let generated_at = format_automation_time(report.generated_at);
    let mode = match report.mode {
        StrategyExecutorRunMode::Preflight => "preflight",
        StrategyExecutorRunMode::Apply => "apply",
    };
    let mut out = format!(
        "executor apply report: report_id={} plan_id={} source_report_id={} generated_at={} mode={} apply_requested={} actions={} would_apply={} applied={} blocked={} skipped={}\n",
        report.report_id,
        report.plan_id,
        report.source_report_id,
        generated_at,
        mode,
        report.apply_requested,
        report.summary.total,
        report.summary.would_apply,
        report.summary.applied,
        report.summary.blocked,
        report.summary.skipped
    );
    if !report.allowlist.is_empty() {
        out.push_str(&format!("allowlist={}\n", report.allowlist.join(",")));
    }
    for action in &report.actions {
        out.push_str(&format!(
            "- id={} suggestion_id={} code={} action_kind={} status={}\n  reason={}\n",
            action.action_id,
            action.suggestion_id,
            action.code,
            strategy_execution_action_kind_name(action.action_kind),
            strategy_executor_action_status_name(action.status),
            action.reason
        ));
    }
    out
}

pub(crate) fn render_strategy_executor_apply_report_markdown(
    report: &StrategyExecutorApplyReport,
    sibling_json: &str,
    plan_json: &str,
) -> String {
    let generated_at = format_automation_time(report.generated_at);
    let mode = match report.mode {
        StrategyExecutorRunMode::Preflight => "preflight",
        StrategyExecutorRunMode::Apply => "apply",
    };
    let mut out = format!(
        concat!(
            "# M12 Executor Apply Report\n\n",
            "- report_id: {}\n",
            "- plan_id: {}\n",
            "- source_report_id: {}\n",
            "- generated_at: {}\n",
            "- mode: {}\n",
            "- apply_requested: {}\n",
            "- allowlist: {}\n",
            "- total: {}\n",
            "- would_apply: {}\n",
            "- applied: {}\n",
            "- blocked: {}\n",
            "- skipped: {}\n",
            "- source_of_truth: {}\n",
            "- plan_json: {}\n\n",
            "> Sibling JSON `{}` is the source of truth. This report is derived from `{}`.\n\n",
            "## Actions\n\n",
        ),
        report.report_id,
        report.plan_id,
        report.source_report_id,
        generated_at,
        mode,
        report.apply_requested,
        if report.allowlist.is_empty() {
            "none".to_string()
        } else {
            report.allowlist.join(",")
        },
        report.summary.total,
        report.summary.would_apply,
        report.summary.applied,
        report.summary.blocked,
        report.summary.skipped,
        sibling_json,
        plan_json,
        sibling_json,
        plan_json
    );
    if report.actions.is_empty() {
        out.push_str("No actions.\n");
        return out;
    }
    for action in &report.actions {
        out.push_str(&format!(
            concat!(
                "### {}\n\n",
                "- suggestion_id: {}\n",
                "- code: {}\n",
                "- action_kind: {}\n",
                "- status: {}\n",
                "- subject: {}\n",
                "- reason: {}\n\n",
            ),
            action.action_id,
            action.suggestion_id,
            action.code,
            strategy_execution_action_kind_name(action.action_kind),
            strategy_executor_action_status_name(action.status),
            action.subject.as_deref().unwrap_or("none"),
            action.reason
        ));
    }
    out
}

pub(crate) fn strategy_report_prefix(generated_at: OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}{:02}{:02}.{:09}Z-m12-suggest",
        generated_at.year(),
        generated_at.month() as u8,
        generated_at.day(),
        generated_at.hour(),
        generated_at.minute(),
        generated_at.second(),
        generated_at.nanosecond()
    )
}

pub(crate) fn parse_outbox_events(ndjson: &str) -> Result<Vec<WikiEvent>, Box<dyn std::error::Error>> {
    let mut events = Vec::new();
    for (idx, line) in ndjson.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let event = serde_json::from_str::<WikiEvent>(line)
            .map_err(|err| format!("outbox event JSON parse error at line {}: {err}", idx + 1))?;
        events.push(event);
    }
    Ok(events)
}
