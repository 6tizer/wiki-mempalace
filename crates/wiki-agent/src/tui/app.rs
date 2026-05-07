use super::message::{MessageBlock, ToolCallStatus};
use super::status::TuiStatus;
use crate::events::ChatEvent;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivePanel {
    Conversation,
    Activity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TuiAction {
    None,
    Submit(String),
    Quit,
}

#[derive(Clone, Debug)]
pub struct TuiApp {
    pub profile: String,
    pub session_id: String,
    pub web_mode: String,
    pub memory_label: String,
    pub backend_label: String,
    pub conversation: Vec<MessageBlock>,
    pub activity: Vec<MessageBlock>,
    pub input: String,
    pub active_panel: ActivePanel,
    pub status: TuiStatus,
    pub history: Vec<String>,
    pub conversation_scroll: u16,
    pub activity_scroll: u16,
    history_index: Option<usize>,
    pub token_count: usize,
    pub tool_count: usize,
    pub latency_ms: Option<u128>,
}

impl TuiApp {
    pub fn new(profile: impl Into<String>) -> Self {
        let profile = profile.into();
        Self {
            profile: profile.clone(),
            session_id: "-".to_string(),
            web_mode: "auto".to_string(),
            memory_label: "session-store".to_string(),
            backend_label: "native".to_string(),
            conversation: Vec::new(),
            activity: vec![MessageBlock::Thinking(format!("profile={profile}"))],
            input: String::new(),
            active_panel: ActivePanel::Conversation,
            status: TuiStatus::Ready,
            history: Vec::new(),
            conversation_scroll: 0,
            activity_scroll: 0,
            history_index: None,
            token_count: 0,
            tool_count: 0,
            latency_ms: None,
        }
    }

    pub fn set_runtime_labels(
        &mut self,
        session_id: impl Into<String>,
        web_mode: impl Into<String>,
        backend_label: impl Into<String>,
    ) {
        self.session_id = session_id.into();
        self.web_mode = web_mode.into();
        self.backend_label = backend_label.into();
    }

    pub fn push_user(&mut self, message: &str) {
        self.conversation
            .push(MessageBlock::User(message.to_string()));
    }

    pub fn push_assistant_message(&mut self, message: &str) {
        self.token_count += count_tokens(message);
        self.conversation
            .push(MessageBlock::AssistantText(message.to_string()));
    }

    pub fn push_assistant_delta(&mut self, delta: &str) {
        self.token_count += count_tokens(delta);
        if let Some(last) = self.conversation.last_mut() {
            if last.append_assistant_delta(delta) {
                return;
            }
        }
        self.conversation
            .push(MessageBlock::AssistantText(delta.to_string()));
    }

    pub fn push_activity(&mut self, line: impl Into<String>) {
        self.activity.push(MessageBlock::Thinking(line.into()));
    }

    pub fn push_error(&mut self, message: impl Into<String>) {
        self.activity.push(MessageBlock::Error(message.into()));
    }

    pub fn push_evidence(&mut self, summary: impl Into<String>) {
        self.activity.push(MessageBlock::Evidence(summary.into()));
    }

    pub fn push_events(&mut self, events: &[ChatEvent]) {
        for event in events {
            match event {
                ChatEvent::ToolStarted { name } => {
                    self.tool_count += 1;
                    self.activity.push(MessageBlock::ToolCall {
                        name: name.clone(),
                        status: ToolCallStatus::Running,
                        duration_ms: None,
                        summary: String::new(),
                        collapsed: true,
                    });
                }
                ChatEvent::ToolFinished {
                    name,
                    duration_ms,
                    summary,
                } => {
                    self.activity.push(MessageBlock::ToolCall {
                        name: name.clone(),
                        status: ToolCallStatus::Succeeded,
                        duration_ms: Some(*duration_ms),
                        summary: summary.clone(),
                        collapsed: true,
                    });
                }
                ChatEvent::ToolFailed {
                    name,
                    duration_ms,
                    error,
                } => {
                    self.activity.push(MessageBlock::ToolCall {
                        name: name.clone(),
                        status: ToolCallStatus::Failed,
                        duration_ms: *duration_ms,
                        summary: error.clone(),
                        collapsed: false,
                    });
                }
                ChatEvent::EvidenceReady { summary } => {
                    self.activity.push(MessageBlock::Evidence(summary.clone()));
                }
                _ => {
                    if let Some(line) = event.activity_line() {
                        self.activity.push(MessageBlock::Thinking(line));
                    }
                }
            }
        }
    }

    pub fn set_status(&mut self, status: TuiStatus) {
        self.status = status;
    }

    pub fn set_latency(&mut self, latency_ms: u128) {
        self.latency_ms = Some(latency_ms);
    }

    pub fn toggle_panel(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::Conversation => ActivePanel::Activity,
            ActivePanel::Activity => ActivePanel::Conversation,
        };
    }

    pub fn scroll_up(&mut self) {
        match self.active_panel {
            ActivePanel::Conversation => {
                self.conversation_scroll = self.conversation_scroll.saturating_sub(1);
            }
            ActivePanel::Activity => {
                self.activity_scroll = self.activity_scroll.saturating_sub(1);
            }
        }
    }

    pub fn scroll_down(&mut self) {
        match self.active_panel {
            ActivePanel::Conversation => {
                self.conversation_scroll = self.conversation_scroll.saturating_add(1);
            }
            ActivePanel::Activity => {
                self.activity_scroll = self.activity_scroll.saturating_add(1);
            }
        }
    }

    pub fn submit_input(&mut self) -> TuiAction {
        let prompt = self.input.trim().to_string();
        if prompt.is_empty() {
            self.input.clear();
            return TuiAction::None;
        }
        self.history.push(prompt.clone());
        self.history_index = None;
        self.input.clear();
        TuiAction::Submit(prompt)
    }

    pub fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let current = self.history_index.unwrap_or(self.history.len());
        let next = current.saturating_sub(1);
        self.history_index = Some(next);
        self.input = self.history[next].clone();
    }

    pub fn history_next(&mut self) {
        let Some(current) = self.history_index else {
            return;
        };
        let next = current + 1;
        if next >= self.history.len() {
            self.history_index = None;
            self.input.clear();
        } else {
            self.history_index = Some(next);
            self.input = self.history[next].clone();
        }
    }

    pub fn status_line(&self) -> String {
        self.footer_line()
    }

    pub fn footer_line(&self) -> String {
        let latency = self
            .latency_ms
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string());
        format!(
            "{} | profile={} | session={} | web={} | memory={} | backend={} | panel={} | tokens={} | tools={} | latency_ms={}",
            self.status.render(),
            self.profile,
            self.session_id,
            self.web_mode,
            self.memory_label,
            self.backend_label,
            match self.active_panel {
                ActivePanel::Conversation => "conversation",
                ActivePanel::Activity => "activity",
            },
            self.token_count,
            self.tool_count,
            latency
        )
    }
}

fn count_tokens(text: &str) -> usize {
    text.split_whitespace().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assistant_delta_appends_to_current_message() {
        let mut app = TuiApp::new("agent_manager");
        app.push_assistant_delta("hello ");
        app.push_assistant_delta("world");
        assert_eq!(
            app.conversation,
            vec![MessageBlock::AssistantText("hello world".to_string())]
        );
        assert_eq!(app.token_count, 2);
    }

    #[test]
    fn history_navigation_round_trips() {
        let mut app = TuiApp::new("agent_manager");
        app.input = "first".into();
        assert_eq!(app.submit_input(), TuiAction::Submit("first".into()));
        app.input = "second".into();
        assert_eq!(app.submit_input(), TuiAction::Submit("second".into()));
        app.history_prev();
        assert_eq!(app.input, "second");
        app.history_prev();
        assert_eq!(app.input, "first");
        app.history_next();
        assert_eq!(app.input, "second");
        app.history_next();
        assert!(app.input.is_empty());
    }

    #[test]
    fn scroll_applies_to_active_panel() {
        let mut app = TuiApp::new("agent_manager");
        app.scroll_down();
        assert_eq!(app.conversation_scroll, 1);
        assert_eq!(app.activity_scroll, 0);
        app.toggle_panel();
        app.scroll_down();
        assert_eq!(app.activity_scroll, 1);
        app.scroll_up();
        assert_eq!(app.activity_scroll, 0);
    }

    #[test]
    fn activity_uses_structured_events_and_counts_started_tools() {
        let mut app = TuiApp::new("agent_manager");
        app.push_events(&[
            ChatEvent::ToolStarted {
                name: "wiki_query".to_string(),
            },
            ChatEvent::ToolFinished {
                name: "wiki_query".to_string(),
                duration_ms: 4,
                summary: "internal_results=1".to_string(),
            },
            ChatEvent::AnswerReady,
        ]);

        assert_eq!(app.tool_count, 1);
        assert!(app.activity.contains(&MessageBlock::ToolCall {
            name: "wiki_query".to_string(),
            status: ToolCallStatus::Running,
            duration_ms: None,
            summary: String::new(),
            collapsed: true,
        }));
        assert!(app.activity.contains(&MessageBlock::ToolCall {
            name: "wiki_query".to_string(),
            status: ToolCallStatus::Succeeded,
            duration_ms: Some(4),
            summary: "internal_results=1".to_string(),
            collapsed: true,
        }));
        assert!(app
            .activity
            .contains(&MessageBlock::Thinking("answer: ready".to_string())));
    }

    #[test]
    fn footer_includes_runtime_labels_and_latency() {
        let mut app = TuiApp::new("agent_manager");
        app.set_runtime_labels("session-1", "always", "native");
        app.set_latency(42);
        app.push_assistant_message("hello world");

        let footer = app.footer_line();
        assert!(footer.contains("profile=agent_manager"));
        assert!(footer.contains("session=session-1"));
        assert!(footer.contains("web=always"));
        assert!(footer.contains("memory=session-store"));
        assert!(footer.contains("backend=native"));
        assert!(footer.contains("tokens=2"));
        assert!(footer.contains("latency_ms=42"));
    }
}
