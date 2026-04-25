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
}
