#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChatEvent {
    UserMessage(String),
    AssistantDelta(String),
    AssistantMessage(String),
    ToolSummary(String),
    Status(String),
    PhaseChanged(String),
    PlanStarted {
        intent: String,
        actions: Vec<String>,
    },
    ToolStarted {
        name: String,
    },
    ToolFinished {
        name: String,
        duration_ms: u128,
        summary: String,
    },
    ToolFailed {
        name: String,
        duration_ms: Option<u128>,
        error: String,
    },
    RetryStarted {
        attempt: usize,
        reason: String,
    },
    EvaluationFinished {
        can_answer: bool,
        reason: String,
    },
    EvidenceReady {
        summary: String,
    },
    AnswerReady,
}

impl ChatEvent {
    pub(crate) fn activity_line(&self) -> Option<String> {
        match self {
            Self::UserMessage(message) => Some(format!("user: chars={}", message.chars().count())),
            Self::AssistantDelta(delta) => {
                Some(format!("assistant_delta: chars={}", delta.chars().count()))
            }
            Self::AssistantMessage(message) => {
                Some(format!("assistant: chars={}", message.chars().count()))
            }
            Self::ToolSummary(summary) | Self::Status(summary) => Some(summary.clone()),
            Self::PhaseChanged(phase) => Some(format!("phase: {phase}")),
            Self::PlanStarted { intent, actions } => Some(format!(
                "plan: intent={intent} actions={}",
                actions.join(",")
            )),
            Self::ToolStarted { name } => Some(format!("tool: {name} started")),
            Self::ToolFinished {
                name,
                duration_ms,
                summary,
            } => Some(format!(
                "tool: {name} finished duration_ms={duration_ms} {summary}"
            )),
            Self::ToolFailed {
                name,
                duration_ms,
                error,
            } => {
                let duration = duration_ms
                    .map(|value| format!(" duration_ms={value}"))
                    .unwrap_or_default();
                Some(format!("tool: {name} failed{duration} error={error}"))
            }
            Self::RetryStarted { attempt, reason } => {
                Some(format!("retry: attempt={attempt} reason={reason}"))
            }
            Self::EvaluationFinished { can_answer, reason } => {
                Some(format!("evaluate: can_answer={can_answer} reason={reason}"))
            }
            Self::EvidenceReady { summary } => Some(format!("evidence: {summary}")),
            Self::AnswerReady => Some("answer: ready".to_string()),
        }
    }
}

pub(crate) fn activity_lines(events: &[ChatEvent]) -> Vec<String> {
    events.iter().filter_map(ChatEvent::activity_line).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_plan_tool_retry_and_evidence_events() {
        let lines = activity_lines(&[
            ChatEvent::PlanStarted {
                intent: "fresh_answer".to_string(),
                actions: vec!["wiki_query".to_string(), "web_search".to_string()],
            },
            ChatEvent::ToolFinished {
                name: "wiki_query".to_string(),
                duration_ms: 12,
                summary: "internal_results=2".to_string(),
            },
            ChatEvent::RetryStarted {
                attempt: 1,
                reason: "low_evidence_retry".to_string(),
            },
            ChatEvent::EvidenceReady {
                summary: "internal=2 web_status=ok web_items=0".to_string(),
            },
            ChatEvent::AnswerReady,
        ]);

        assert_eq!(
            lines,
            vec![
                "plan: intent=fresh_answer actions=wiki_query,web_search",
                "tool: wiki_query finished duration_ms=12 internal_results=2",
                "retry: attempt=1 reason=low_evidence_retry",
                "evidence: internal=2 web_status=ok web_items=0",
                "answer: ready",
            ]
        );
    }
}
