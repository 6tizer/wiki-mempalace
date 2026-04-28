//! Strategy suggestion report model.
//!
//! Pure data model only: no DB, outbox, projection, or report file IO.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrategyReport {
    pub report_id: String,
    pub generated_at: Option<OffsetDateTime>,
    pub viewer_scope: Option<String>,
    pub suggestions: Vec<StrategySuggestion>,
}

impl StrategyReport {
    pub fn empty(report_id: impl Into<String>) -> Self {
        Self {
            report_id: report_id.into(),
            generated_at: None,
            viewer_scope: None,
            suggestions: Vec::new(),
        }
    }

    pub fn new(
        report_id: impl Into<String>,
        generated_at: Option<OffsetDateTime>,
        viewer_scope: Option<String>,
        suggestions: Vec<StrategySuggestion>,
    ) -> Self {
        Self {
            report_id: report_id.into(),
            generated_at,
            viewer_scope,
            suggestions,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrategySuggestion {
    pub suggestion_id: String,
    pub code: String,
    pub severity: StrategySeverity,
    pub subject: Option<String>,
    pub reason: String,
    pub suggested_command: Option<String>,
    pub execution_policy: StrategyExecutionPolicy,
}

impl StrategySuggestion {
    pub fn new(
        suggestion_id: impl Into<String>,
        code: impl Into<String>,
        severity: StrategySeverity,
        reason: impl Into<String>,
        execution_policy: StrategyExecutionPolicy,
    ) -> Self {
        Self {
            suggestion_id: suggestion_id.into(),
            code: code.into(),
            severity,
            subject: None,
            reason: reason.into(),
            suggested_command: None,
            execution_policy,
        }
    }

    pub fn with_subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    pub fn with_suggested_command(mut self, suggested_command: impl Into<String>) -> Self {
        self.suggested_command = Some(suggested_command.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategySeverity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategyExecutionPolicy {
    AutoSafe,
    AgentReview,
    HumanRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrategyExecutionPlan {
    pub plan_id: String,
    pub source_report_id: String,
    pub generated_at: Option<OffsetDateTime>,
    pub viewer_scope: Option<String>,
    pub mode: StrategyExecutionPlanMode,
    pub actions: Vec<StrategyExecutionAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrategyExecutionAction {
    pub action_id: String,
    pub suggestion_id: String,
    pub code: String,
    pub subject: Option<String>,
    pub execution_policy: StrategyExecutionPolicy,
    pub action_kind: StrategyExecutionActionKind,
    pub dry_run_status: StrategyExecutionDryRunStatus,
    pub command_preview: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategyExecutionPlanMode {
    DryRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategyExecutionActionKind {
    FixAutoSafe,
    AgentReview,
    HumanRequired,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategyExecutionDryRunStatus {
    WouldApply,
    Blocked,
}

pub fn build_strategy_execution_plan(report: &StrategyReport) -> StrategyExecutionPlan {
    let plan_id = format!("{}-executor-plan", report.report_id);
    let actions = report
        .suggestions
        .iter()
        .enumerate()
        .map(|(idx, suggestion)| {
            let (action_kind, dry_run_status, reason) = plan_action(suggestion);
            StrategyExecutionAction {
                action_id: format!("{plan_id}-{:04}", idx + 1),
                suggestion_id: suggestion.suggestion_id.clone(),
                code: suggestion.code.clone(),
                subject: suggestion.subject.clone(),
                execution_policy: suggestion.execution_policy,
                action_kind,
                dry_run_status,
                command_preview: suggestion.suggested_command.clone(),
                reason: reason.to_string(),
            }
        })
        .collect();

    StrategyExecutionPlan {
        plan_id,
        source_report_id: report.report_id.clone(),
        generated_at: report.generated_at,
        viewer_scope: report.viewer_scope.clone(),
        mode: StrategyExecutionPlanMode::DryRun,
        actions,
    }
}

fn plan_action(
    suggestion: &StrategySuggestion,
) -> (
    StrategyExecutionActionKind,
    StrategyExecutionDryRunStatus,
    &'static str,
) {
    match (
        suggestion.execution_policy,
        suggestion.code.as_str(),
        suggestion.suggested_command.as_deref(),
    ) {
        (
            StrategyExecutionPolicy::AutoSafe,
            "suggest.fix_auto_safe",
            Some("cargo run -p wiki-cli -- fix --write --auto-only"),
        ) => (
            StrategyExecutionActionKind::FixAutoSafe,
            StrategyExecutionDryRunStatus::WouldApply,
            "auto_safe suggestion is eligible for a future guarded apply allowlist; dry-run does not execute it",
        ),
        (StrategyExecutionPolicy::AutoSafe, _, _) => (
            StrategyExecutionActionKind::Unsupported,
            StrategyExecutionDryRunStatus::Blocked,
            "auto_safe suggestion is not in the dry-run planner allowlist",
        ),
        (StrategyExecutionPolicy::AgentReview, _, _) => (
            StrategyExecutionActionKind::AgentReview,
            StrategyExecutionDryRunStatus::Blocked,
            "agent_review suggestion requires separate review before execution",
        ),
        (StrategyExecutionPolicy::HumanRequired, _, _) => (
            StrategyExecutionActionKind::HumanRequired,
            StrategyExecutionDryRunStatus::Blocked,
            "human_required suggestion is outside executor automation",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_report_serializes_stable_shape() {
        let report = StrategyReport::empty("m12-test-report");
        let json = serde_json::to_value(report).unwrap();

        assert_eq!(json["report_id"], "m12-test-report");
        assert_eq!(json["generated_at"], serde_json::Value::Null);
        assert_eq!(json["viewer_scope"], serde_json::Value::Null);
        assert_eq!(json["suggestions"], serde_json::json!([]));
    }

    #[test]
    fn severity_and_execution_policy_serialize_as_snake_case() {
        assert_eq!(
            serde_json::to_value(StrategySeverity::Low).unwrap(),
            serde_json::json!("low")
        );
        assert_eq!(
            serde_json::to_value(StrategySeverity::Medium).unwrap(),
            serde_json::json!("medium")
        );
        assert_eq!(
            serde_json::to_value(StrategySeverity::High).unwrap(),
            serde_json::json!("high")
        );
        assert_eq!(
            serde_json::to_value(StrategyExecutionPolicy::AutoSafe).unwrap(),
            serde_json::json!("auto_safe")
        );
        assert_eq!(
            serde_json::to_value(StrategyExecutionPolicy::AgentReview).unwrap(),
            serde_json::json!("agent_review")
        );
        assert_eq!(
            serde_json::to_value(StrategyExecutionPolicy::HumanRequired).unwrap(),
            serde_json::json!("human_required")
        );
    }

    #[test]
    fn suggestion_shape_contains_code_reason_and_execution_policy() {
        let suggestion = StrategySuggestion::new(
            "sug-1",
            "suggest.crystallize_candidate",
            StrategySeverity::Medium,
            "Crystallize repeated query result",
            StrategyExecutionPolicy::AgentReview,
        );
        let json = serde_json::to_value(suggestion).unwrap();

        assert_eq!(json["suggestion_id"], "sug-1");
        assert_eq!(json["code"], "suggest.crystallize_candidate");
        assert_eq!(json["reason"], "Crystallize repeated query result");
        assert_eq!(json["execution_policy"], "agent_review");
    }

    #[test]
    fn dry_run_plan_marks_auto_safe_as_would_apply() {
        let report = StrategyReport::new(
            "report-1",
            None,
            Some("private:cli".to_string()),
            vec![
                StrategySuggestion::new(
                    "sug-1",
                    "suggest.fix_auto_safe",
                    StrategySeverity::Low,
                    "Auto fix available",
                    StrategyExecutionPolicy::AutoSafe,
                )
                .with_suggested_command("cargo run -p wiki-cli -- fix --write --auto-only"),
                StrategySuggestion::new(
                    "sug-2",
                    "suggest.crystallize_candidate",
                    StrategySeverity::Medium,
                    "Crystallize candidate",
                    StrategyExecutionPolicy::AgentReview,
                )
                .with_suggested_command("cargo run -p wiki-cli -- crystallize \"<redacted>\""),
            ],
        );

        let plan = build_strategy_execution_plan(&report);

        assert_eq!(plan.plan_id, "report-1-executor-plan");
        assert_eq!(plan.source_report_id, "report-1");
        assert_eq!(plan.mode, StrategyExecutionPlanMode::DryRun);
        assert_eq!(plan.actions.len(), 2);
        assert_eq!(
            plan.actions[0].dry_run_status,
            StrategyExecutionDryRunStatus::WouldApply
        );
        assert_eq!(
            plan.actions[0].action_kind,
            StrategyExecutionActionKind::FixAutoSafe
        );
        assert_eq!(
            plan.actions[1].dry_run_status,
            StrategyExecutionDryRunStatus::Blocked
        );
        assert_eq!(
            plan.actions[1].action_kind,
            StrategyExecutionActionKind::AgentReview
        );
    }

    #[test]
    fn dry_run_plan_serializes_stable_shape() {
        let report = StrategyReport::new(
            "report-1",
            None,
            None,
            vec![StrategySuggestion::new(
                "sug-1",
                "suggest.manual",
                StrategySeverity::High,
                "Needs human",
                StrategyExecutionPolicy::HumanRequired,
            )],
        );

        let json = serde_json::to_value(build_strategy_execution_plan(&report)).unwrap();

        assert_eq!(json["plan_id"], "report-1-executor-plan");
        assert_eq!(json["source_report_id"], "report-1");
        assert_eq!(json["mode"], "dry_run");
        assert_eq!(
            json["actions"][0]["action_id"],
            "report-1-executor-plan-0001"
        );
        assert_eq!(json["actions"][0]["dry_run_status"], "blocked");
        assert_eq!(json["actions"][0]["action_kind"], "human_required");
        assert_eq!(
            json["actions"][0]["execution_policy"],
            serde_json::json!("human_required")
        );
    }

    #[test]
    fn dry_run_plan_blocks_unknown_auto_safe_commands() {
        let report = StrategyReport::new(
            "report-1",
            None,
            None,
            vec![StrategySuggestion::new(
                "sug-1",
                "suggest.future_auto",
                StrategySeverity::Low,
                "Future auto action",
                StrategyExecutionPolicy::AutoSafe,
            )
            .with_suggested_command("cargo run -p wiki-cli -- future --write")],
        );

        let plan = build_strategy_execution_plan(&report);

        assert_eq!(
            plan.actions[0].dry_run_status,
            StrategyExecutionDryRunStatus::Blocked
        );
        assert_eq!(
            plan.actions[0].action_kind,
            StrategyExecutionActionKind::Unsupported
        );
    }
}
