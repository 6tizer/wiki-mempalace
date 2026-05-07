use crate::evaluator::{evaluate_evidence, EvaluationReport};
use crate::events::ChatEvent;
use crate::evidence::{EvidencePack, InternalEvidence};
use crate::planner::TaskPlan;
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HarnessPhase {
    Plan,
    Act,
    Observe,
    Evaluate,
    Answer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HarnessAction {
    WikiQuery {
        query: String,
        per_stream_limit: usize,
    },
    WebSearchPolicy {
        query: String,
    },
    Answer {
        query: String,
    },
}

impl HarnessAction {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::WikiQuery { .. } => "wiki_query",
            Self::WebSearchPolicy { .. } => "web_search_policy",
            Self::Answer { .. } => "answer",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HarnessPlan {
    pub(crate) intent: String,
    pub(crate) actions: Vec<HarnessAction>,
    pub(crate) planned_tools: Vec<String>,
}

impl HarnessPlan {
    pub(crate) fn for_task(prompt: &str, task_plan: &TaskPlan) -> Self {
        let query = prompt.to_string();
        Self {
            intent: task_plan.intent.render().to_string(),
            actions: vec![
                HarnessAction::WikiQuery {
                    query: query.clone(),
                    per_stream_limit: task_plan.evidence_budget.local_limit,
                },
                HarnessAction::WebSearchPolicy {
                    query: query.clone(),
                },
                HarnessAction::Answer { query },
            ],
            planned_tools: task_plan.tool_names(),
        }
    }

    fn plan_items(&self) -> Vec<String> {
        self.planned_tools.clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HarnessObservation {
    pub(crate) action: String,
    pub(crate) summary: String,
}

#[derive(Clone, Debug)]
pub(crate) struct HarnessTurnResult {
    pub(crate) evidence: EvidencePack,
    pub(crate) evidence_rendered: String,
    pub(crate) answer: String,
    pub(crate) events: Vec<ChatEvent>,
    pub(crate) observations: Vec<HarnessObservation>,
    pub(crate) evaluation: EvaluationReport,
}

pub(crate) trait HarnessDelegate {
    fn wiki_query(
        &mut self,
        query: &str,
        per_stream_limit: usize,
    ) -> Result<Vec<InternalEvidence>, Box<dyn std::error::Error>>;

    fn web_search_policy(
        &mut self,
        query: &str,
        internal: Vec<InternalEvidence>,
    ) -> Result<EvidencePack, Box<dyn std::error::Error>>;

    fn answer(
        &mut self,
        query: &str,
        evidence: &EvidencePack,
    ) -> Result<String, Box<dyn std::error::Error>>;
}

#[derive(Default)]
pub(crate) struct HarnessRuntime;

impl HarnessRuntime {
    pub(crate) fn run(
        &self,
        prompt: &str,
        task_plan: &TaskPlan,
        delegate: &mut impl HarnessDelegate,
    ) -> Result<HarnessTurnResult, Box<dyn std::error::Error>> {
        let mut events = Vec::new();
        let mut observations = Vec::new();

        let phase = HarnessPhase::Plan;
        events.push(ChatEvent::PhaseChanged(phase.render().to_string()));
        let plan = HarnessPlan::for_task(prompt, task_plan);
        events.push(ChatEvent::PlanStarted {
            intent: plan.intent.clone(),
            actions: plan.plan_items(),
        });

        events.push(ChatEvent::PhaseChanged(
            HarnessPhase::Act.render().to_string(),
        ));
        let mut internal = Vec::new();
        let mut evidence = None;
        let mut evaluation = None;
        let mut retry_count = 0;
        let mut answer = String::new();

        for action in plan.actions {
            let action_name = action.name().to_string();
            events.push(ChatEvent::ToolStarted {
                name: action_name.clone(),
            });
            match action {
                HarnessAction::WikiQuery {
                    query,
                    per_stream_limit,
                } => {
                    let started_at = Instant::now();
                    match delegate.wiki_query(&query, per_stream_limit) {
                        Ok(results) => {
                            internal = results;
                        }
                        Err(err) => {
                            events.push(ChatEvent::ToolFailed {
                                name: action_name.clone(),
                                duration_ms: Some(started_at.elapsed().as_millis()),
                                error: err.to_string(),
                            });
                            internal = Vec::new();
                        }
                    }
                    let summary = format!("internal_results={}", internal.len());
                    observations.push(HarnessObservation {
                        action: action_name.clone(),
                        summary: summary.clone(),
                    });
                    events.push(ChatEvent::ToolFinished {
                        name: action_name,
                        duration_ms: started_at.elapsed().as_millis(),
                        summary,
                    });
                }
                HarnessAction::WebSearchPolicy { query } => {
                    let started_at = Instant::now();
                    let mut pack = web_search_or_degraded(
                        delegate,
                        &query,
                        std::mem::take(&mut internal),
                        &mut events,
                    );
                    let mut report = evaluate_evidence(&pack, task_plan, retry_count);
                    while report.should_retry {
                        retry_count += 1;
                        events.push(ChatEvent::RetryStarted {
                            attempt: retry_count,
                            reason: report.reason.clone(),
                        });
                        let retry_limit = task_plan.evidence_budget.local_limit * (retry_count + 1);
                        let retry_tool_name = "wiki_query".to_string();
                        events.push(ChatEvent::ToolStarted {
                            name: retry_tool_name.clone(),
                        });
                        let started_at = Instant::now();
                        let retry_internal = match delegate.wiki_query(&query, retry_limit) {
                            Ok(results) => {
                                events.push(ChatEvent::ToolFinished {
                                    name: retry_tool_name,
                                    duration_ms: started_at.elapsed().as_millis(),
                                    summary: format!("internal_results={}", results.len()),
                                });
                                results
                            }
                            Err(err) => {
                                events.push(ChatEvent::ToolFailed {
                                    name: retry_tool_name,
                                    duration_ms: Some(started_at.elapsed().as_millis()),
                                    error: err.to_string(),
                                });
                                Vec::new()
                            }
                        };
                        pack =
                            web_search_or_degraded(delegate, &query, retry_internal, &mut events);
                        report = evaluate_evidence(&pack, task_plan, retry_count);
                    }
                    let summary =
                        format!("{} evaluation={}", web_policy_summary(&pack), report.reason);
                    observations.push(HarnessObservation {
                        action: action_name.clone(),
                        summary: summary.clone(),
                    });
                    events.push(ChatEvent::ToolFinished {
                        name: action_name,
                        duration_ms: started_at.elapsed().as_millis(),
                        summary,
                    });
                    events.push(ChatEvent::PhaseChanged(
                        HarnessPhase::Observe.render().to_string(),
                    ));
                    events.push(ChatEvent::EvidenceReady {
                        summary: evidence_summary(&pack),
                    });
                    evaluation = Some(report);
                    evidence = Some(pack);
                }
                HarnessAction::Answer { query } => {
                    let Some(pack) = evidence.as_ref() else {
                        return Err("harness answer step missing evidence".into());
                    };
                    events.push(ChatEvent::PhaseChanged(
                        HarnessPhase::Evaluate.render().to_string(),
                    ));
                    let evaluation = evaluation
                        .clone()
                        .unwrap_or_else(|| evaluate_evidence(pack, task_plan, retry_count));
                    events.push(ChatEvent::EvaluationFinished {
                        can_answer: evaluation.can_answer,
                        reason: evaluation.reason.clone(),
                    });
                    events.push(ChatEvent::PhaseChanged(
                        HarnessPhase::Answer.render().to_string(),
                    ));
                    let started_at = Instant::now();
                    answer = delegate.answer(&query, pack)?;
                    events.push(ChatEvent::ToolFinished {
                        name: action_name,
                        duration_ms: started_at.elapsed().as_millis(),
                        summary: format!("answer_chars={}", answer.chars().count()),
                    });
                    events.push(ChatEvent::AnswerReady);
                }
            }
        }

        let evidence = evidence.ok_or("harness completed without evidence")?;
        let evidence_rendered = evidence.render();
        let evaluation =
            evaluation.unwrap_or_else(|| evaluate_evidence(&evidence, task_plan, retry_count));
        Ok(HarnessTurnResult {
            evidence,
            evidence_rendered,
            answer,
            events,
            observations,
            evaluation,
        })
    }
}

impl HarnessPhase {
    fn render(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Act => "act",
            Self::Observe => "observe",
            Self::Evaluate => "evaluate",
            Self::Answer => "answer",
        }
    }
}

fn web_policy_summary(pack: &EvidencePack) -> String {
    if let Some(web) = &pack.web {
        return format!(
            "web_results={} providers={} domains={}",
            web.evidence.len(),
            web.providers_succeeded.len(),
            web.distinct_domains
        );
    }
    format!(
        "web_status={}",
        pack.web_status.as_deref().unwrap_or("not requested")
    )
}

fn evidence_summary(pack: &EvidencePack) -> String {
    format!(
        "internal={} web_status={} web_items={}",
        pack.internal.len(),
        pack.web_status.as_deref().unwrap_or("ok"),
        pack.web.as_ref().map(|web| web.evidence.len()).unwrap_or(0)
    )
}

fn web_search_or_degraded(
    delegate: &mut impl HarnessDelegate,
    query: &str,
    internal: Vec<InternalEvidence>,
    events: &mut Vec<ChatEvent>,
) -> EvidencePack {
    let started_at = Instant::now();
    match delegate.web_search_policy(query, internal.clone()) {
        Ok(pack) => pack,
        Err(err) => {
            events.push(ChatEvent::ToolFailed {
                name: "web_search_policy".to_string(),
                duration_ms: Some(started_at.elapsed().as_millis()),
                error: err.to_string(),
            });
            let mut pack = EvidencePack::new(internal);
            pack.web_status = Some(format!("failed: {err}"));
            pack
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::{TaskPlan, WebMode};

    struct FakeDelegate {
        evidence: EvidencePack,
        answer: String,
        retry_internal: Option<Vec<InternalEvidence>>,
        web_error: Option<String>,
        wiki_calls: usize,
    }

    impl HarnessDelegate for FakeDelegate {
        fn wiki_query(
            &mut self,
            _query: &str,
            _per_stream_limit: usize,
        ) -> Result<Vec<InternalEvidence>, Box<dyn std::error::Error>> {
            self.wiki_calls += 1;
            if self.wiki_calls > 1 {
                if let Some(results) = &self.retry_internal {
                    return Ok(results.clone());
                }
            }
            Ok(self.evidence.internal.clone())
        }

        fn web_search_policy(
            &mut self,
            _query: &str,
            internal: Vec<InternalEvidence>,
        ) -> Result<EvidencePack, Box<dyn std::error::Error>> {
            if let Some(error) = &self.web_error {
                return Err(error.clone().into());
            }
            let mut pack = self.evidence.clone();
            pack.internal = internal;
            Ok(pack)
        }

        fn answer(
            &mut self,
            _query: &str,
            _evidence: &EvidencePack,
        ) -> Result<String, Box<dyn std::error::Error>> {
            Ok(self.answer.clone())
        }
    }

    #[test]
    fn harness_runs_ordered_loop() {
        let evidence = EvidencePack::new(vec![InternalEvidence {
            doc_id: "page:1".to_string(),
            score: 1.0,
            title: Some("Title".to_string()),
            excerpt: Some("Excerpt".to_string()),
        }]);
        let mut delegate = FakeDelegate {
            evidence,
            answer: "done".to_string(),
            retry_internal: None,
            web_error: None,
            wiki_calls: 0,
        };
        let task_plan = TaskPlan::for_chat("question", WebMode::Auto, "shared:wiki", false);

        let result = HarnessRuntime
            .run("question", &task_plan, &mut delegate)
            .expect("harness run");

        assert_eq!(result.answer, "done");
        assert!(result.evaluation.can_answer);
        assert_eq!(
            result.observations[0],
            HarnessObservation {
                action: "wiki_query".to_string(),
                summary: "internal_results=1".to_string()
            }
        );
        assert!(result.events.contains(&ChatEvent::AnswerReady));
    }

    #[test]
    fn harness_retries_low_evidence_once() {
        let retry_internal = vec![InternalEvidence {
            doc_id: "page:retry".to_string(),
            score: 1.0,
            title: None,
            excerpt: None,
        }];
        let mut delegate = FakeDelegate {
            evidence: EvidencePack::new(Vec::new()),
            answer: "done".to_string(),
            retry_internal: Some(retry_internal),
            web_error: None,
            wiki_calls: 0,
        };
        let task_plan = TaskPlan::for_chat("question", WebMode::Off, "shared:wiki", false);

        let result = HarnessRuntime
            .run("question", &task_plan, &mut delegate)
            .expect("harness run");

        assert_eq!(delegate.wiki_calls, 2);
        assert!(result.evaluation.can_answer);
        assert!(result.events.contains(&ChatEvent::RetryStarted {
            attempt: 1,
            reason: "low_evidence_retry".to_string()
        }));
    }

    #[test]
    fn harness_degrades_web_failure_to_local_evidence() {
        let evidence = EvidencePack::new(vec![InternalEvidence {
            doc_id: "page:1".to_string(),
            score: 1.0,
            title: None,
            excerpt: None,
        }]);
        let mut delegate = FakeDelegate {
            evidence,
            answer: "done".to_string(),
            retry_internal: None,
            web_error: Some("web unavailable".to_string()),
            wiki_calls: 0,
        };
        let task_plan = TaskPlan::for_chat("latest release", WebMode::Always, "shared:wiki", false);

        let result = HarnessRuntime
            .run("latest release", &task_plan, &mut delegate)
            .expect("harness run");

        assert!(result.evaluation.can_answer);
        assert_eq!(
            result.evidence.web_status.as_deref(),
            Some("failed: web unavailable")
        );
        assert!(result.events.iter().any(|event| matches!(
            event,
            ChatEvent::ToolFailed { name, error, .. }
                if name == "web_search_policy" && error == "web unavailable"
        )));
    }
}
