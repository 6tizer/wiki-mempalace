use crate::events::ChatEvent;
use crate::evidence::{EvidencePack, InternalEvidence};
use crate::planner::TaskPlan;

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HarnessEvaluation {
    pub(crate) can_answer: bool,
    pub(crate) reason: String,
}

impl HarnessEvaluation {
    fn from_evidence(evidence: &EvidencePack) -> Self {
        let has_internal = !evidence.internal.is_empty();
        let has_web = evidence
            .web
            .as_ref()
            .is_some_and(|web| !web.evidence.is_empty());
        let web_blocked_or_off = evidence.web_status.is_some();
        let can_answer = has_internal || has_web || web_blocked_or_off;
        let reason = if has_internal || has_web {
            "evidence_available"
        } else if web_blocked_or_off {
            "web_unavailable_with_local_result"
        } else {
            "no_evidence"
        };
        Self {
            can_answer,
            reason: reason.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct HarnessTurnResult {
    pub(crate) evidence: EvidencePack,
    pub(crate) evidence_rendered: String,
    pub(crate) answer: String,
    pub(crate) events: Vec<ChatEvent>,
    pub(crate) observations: Vec<HarnessObservation>,
    pub(crate) evaluation: HarnessEvaluation,
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
                    internal = delegate.wiki_query(&query, per_stream_limit)?;
                    let summary = format!("internal_results={}", internal.len());
                    observations.push(HarnessObservation {
                        action: action_name.clone(),
                        summary: summary.clone(),
                    });
                    events.push(ChatEvent::ToolFinished {
                        name: action_name,
                        summary,
                    });
                }
                HarnessAction::WebSearchPolicy { query } => {
                    let pack = delegate.web_search_policy(&query, std::mem::take(&mut internal))?;
                    let summary = web_policy_summary(&pack);
                    observations.push(HarnessObservation {
                        action: action_name.clone(),
                        summary: summary.clone(),
                    });
                    events.push(ChatEvent::ToolFinished {
                        name: action_name,
                        summary,
                    });
                    events.push(ChatEvent::PhaseChanged(
                        HarnessPhase::Observe.render().to_string(),
                    ));
                    evidence = Some(pack);
                }
                HarnessAction::Answer { query } => {
                    let Some(pack) = evidence.as_ref() else {
                        return Err("harness answer step missing evidence".into());
                    };
                    events.push(ChatEvent::PhaseChanged(
                        HarnessPhase::Evaluate.render().to_string(),
                    ));
                    let evaluation = HarnessEvaluation::from_evidence(pack);
                    events.push(ChatEvent::EvaluationFinished {
                        can_answer: evaluation.can_answer,
                        reason: evaluation.reason.clone(),
                    });
                    events.push(ChatEvent::PhaseChanged(
                        HarnessPhase::Answer.render().to_string(),
                    ));
                    answer = delegate.answer(&query, pack)?;
                    events.push(ChatEvent::ToolFinished {
                        name: action_name,
                        summary: format!("answer_chars={}", answer.chars().count()),
                    });
                    events.push(ChatEvent::AnswerReady);
                }
            }
        }

        let evidence = evidence.ok_or("harness completed without evidence")?;
        let evidence_rendered = evidence.render();
        let evaluation = HarnessEvaluation::from_evidence(&evidence);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::{TaskPlan, WebMode};

    struct FakeDelegate {
        evidence: EvidencePack,
        answer: String,
    }

    impl HarnessDelegate for FakeDelegate {
        fn wiki_query(
            &mut self,
            _query: &str,
            _per_stream_limit: usize,
        ) -> Result<Vec<InternalEvidence>, Box<dyn std::error::Error>> {
            Ok(self.evidence.internal.clone())
        }

        fn web_search_policy(
            &mut self,
            _query: &str,
            internal: Vec<InternalEvidence>,
        ) -> Result<EvidencePack, Box<dyn std::error::Error>> {
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
}
