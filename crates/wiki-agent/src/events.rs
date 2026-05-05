#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChatEvent {
    UserMessage(String),
    AssistantDelta(String),
    AssistantMessage(String),
    ToolSummary(String),
    Status(String),
}
