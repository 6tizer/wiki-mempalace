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
        summary: String,
    },
    ToolFailed {
        name: String,
        error: String,
    },
    EvaluationFinished {
        can_answer: bool,
        reason: String,
    },
    AnswerReady,
}
