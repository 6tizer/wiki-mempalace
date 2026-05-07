use crate::evidence::EvidencePack;
use crate::planner::TaskPlan;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvaluationReport {
    pub(crate) can_answer: bool,
    pub(crate) should_retry: bool,
    pub(crate) reason: String,
}

pub(crate) fn evaluate_evidence(
    evidence: &EvidencePack,
    task_plan: &TaskPlan,
    retry_count: usize,
) -> EvaluationReport {
    let has_internal = !evidence.internal.is_empty();
    let web = evidence.web.as_ref();
    let has_web = web.is_some_and(|run| !run.evidence.is_empty());
    let web_not_cross_verified = web.is_some_and(|run| !run.cross_verified)
        && task_plan.evidence_budget.require_cross_verified_web;
    let can_answer = has_internal || (has_web && !web_not_cross_verified);
    let low_evidence = !has_internal && !has_web;
    let tool_failure = evidence
        .web_status
        .as_deref()
        .is_some_and(|status| status.starts_with("failed:"));
    let should_retry = ((low_evidence && task_plan.retry_policy.retry_on_low_evidence)
        || (tool_failure && !has_internal && task_plan.retry_policy.retry_on_tool_failure))
        && retry_count < task_plan.retry_policy.max_retries;
    let reason = if should_retry && tool_failure {
        "tool_failure_retry"
    } else if should_retry {
        "low_evidence_retry"
    } else if web_not_cross_verified && has_internal {
        "web_not_cross_verified_degraded_to_local"
    } else if web_not_cross_verified {
        "web_not_cross_verified"
    } else if has_internal || has_web {
        "evidence_available"
    } else {
        "no_supported_evidence"
    };
    EvaluationReport {
        can_answer,
        should_retry,
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::InternalEvidence;
    use crate::planner::WebMode;
    use wiki_ai::web_search::{WebSearchEvidence, WebSearchRun};

    #[test]
    fn retries_low_evidence_within_budget() {
        let plan = TaskPlan::for_chat("question", WebMode::Off, "shared:wiki", false);
        let report = evaluate_evidence(&EvidencePack::new(Vec::new()), &plan, 0);
        assert!(!report.can_answer);
        assert!(report.should_retry);
        assert_eq!(report.reason, "low_evidence_retry");
    }

    #[test]
    fn accepts_internal_evidence() {
        let plan = TaskPlan::for_chat("question", WebMode::Off, "shared:wiki", false);
        let report = evaluate_evidence(
            &EvidencePack::new(vec![InternalEvidence {
                doc_id: "page:1".to_string(),
                score: 1.0,
                title: None,
                excerpt: None,
            }]),
            &plan,
            0,
        );
        assert!(report.can_answer);
        assert!(!report.should_retry);
        assert_eq!(report.reason, "evidence_available");
    }

    #[test]
    fn rejects_uncross_verified_required_web() {
        let plan = TaskPlan::for_chat("latest release", WebMode::Always, "shared:wiki", false);
        let mut evidence = EvidencePack::new(Vec::new());
        evidence.web = Some(WebSearchRun {
            query: "latest release".to_string(),
            providers_requested: vec!["exa".to_string()],
            providers_succeeded: vec!["exa".to_string()],
            providers_failed: Vec::new(),
            distinct_domains: 1,
            cross_verified: false,
            evidence: vec![web_evidence()],
        });
        let report = evaluate_evidence(&evidence, &plan, 0);
        assert!(!report.can_answer);
        assert_eq!(report.reason, "web_not_cross_verified");
    }

    #[test]
    fn degrades_uncross_verified_web_to_internal_evidence() {
        let plan = TaskPlan::for_chat("latest release", WebMode::Always, "shared:wiki", false);
        let mut evidence = EvidencePack::new(vec![InternalEvidence {
            doc_id: "page:1".to_string(),
            score: 1.0,
            title: None,
            excerpt: None,
        }]);
        evidence.web = Some(WebSearchRun {
            query: "latest release".to_string(),
            providers_requested: vec!["exa".to_string()],
            providers_succeeded: vec!["exa".to_string()],
            providers_failed: Vec::new(),
            distinct_domains: 1,
            cross_verified: false,
            evidence: vec![web_evidence()],
        });
        let report = evaluate_evidence(&evidence, &plan, 0);
        assert!(report.can_answer);
        assert_eq!(report.reason, "web_not_cross_verified_degraded_to_local");
    }

    #[test]
    fn retries_tool_failure_without_internal_evidence() {
        let plan = TaskPlan::for_chat("latest release", WebMode::Always, "shared:wiki", false);
        let mut evidence = EvidencePack::new(Vec::new());
        evidence.web_status = Some("failed: provider unavailable".to_string());
        let report = evaluate_evidence(&evidence, &plan, 0);
        assert!(!report.can_answer);
        assert!(report.should_retry);
        assert_eq!(report.reason, "tool_failure_retry");
    }

    fn web_evidence() -> WebSearchEvidence {
        WebSearchEvidence {
            query: "latest release".to_string(),
            provider: "exa".to_string(),
            title: "Release".to_string(),
            url: "https://example.com/release".to_string(),
            domain: "example.com".to_string(),
            snippet: "release notes".to_string(),
            summary: None,
            retrieved_at: "2026-05-07T00:00:00Z".to_string(),
            content_hash: "hash".to_string(),
        }
    }
}
