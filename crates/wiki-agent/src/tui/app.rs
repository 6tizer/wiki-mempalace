use super::status::TuiStatus;
use crate::events::{activity_lines, ChatEvent};

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
    pub conversation: Vec<String>,
    pub activity: Vec<String>,
    pub input: String,
    pub active_panel: ActivePanel,
    pub status: TuiStatus,
    pub history: Vec<String>,
    pub conversation_scroll: u16,
    pub activity_scroll: u16,
    history_index: Option<usize>,
    pub token_count: usize,
    pub tool_count: usize,
}

impl TuiApp {
    pub fn new(profile: impl Into<String>) -> Self {
        let profile = profile.into();
        Self {
            conversation: Vec::new(),
            activity: vec![format!("profile={profile}")],
            input: String::new(),
            active_panel: ActivePanel::Conversation,
            status: TuiStatus::Ready,
            history: Vec::new(),
            conversation_scroll: 0,
            activity_scroll: 0,
            history_index: None,
            token_count: 0,
            tool_count: 0,
        }
    }

    pub fn push_user(&mut self, message: &str) {
        self.conversation.push(format!("user: {message}"));
    }

    pub fn push_assistant_message(&mut self, message: &str) {
        self.token_count += count_tokens(message);
        self.conversation.push(format!("assistant: {message}"));
    }

    pub fn push_assistant_delta(&mut self, delta: &str) {
        self.token_count += count_tokens(delta);
        if let Some(last) = self.conversation.last_mut() {
            if last.starts_with("assistant: ") {
                last.push_str(delta);
                return;
            }
        }
        self.conversation.push(format!("assistant: {delta}"));
    }

    pub fn push_activity(&mut self, line: impl Into<String>) {
        self.activity.push(line.into());
    }

    pub fn push_events(&mut self, events: &[ChatEvent]) {
        for event in events {
            if matches!(event, ChatEvent::ToolStarted { .. }) {
                self.tool_count += 1;
            }
        }
        for line in activity_lines(events) {
            self.push_activity(line);
        }
    }

    pub fn set_status(&mut self, status: TuiStatus) {
        self.status = status;
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
        format!(
            "{} | panel={} | tokens={} | tools={}",
            self.status.render(),
            match self.active_panel {
                ActivePanel::Conversation => "conversation",
                ActivePanel::Activity => "activity",
            },
            self.token_count,
            self.tool_count
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
        assert_eq!(app.conversation, vec!["assistant: hello world"]);
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
        assert!(app
            .activity
            .contains(&"tool: wiki_query started".to_string()));
        assert!(app
            .activity
            .contains(&"tool: wiki_query finished duration_ms=4 internal_results=1".to_string()));
        assert!(app.activity.contains(&"answer: ready".to_string()));
    }
}
