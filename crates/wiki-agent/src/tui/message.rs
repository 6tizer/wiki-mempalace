use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolCallStatus {
    Running,
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MessageBlock {
    User(String),
    AssistantText(String),
    Thinking(String),
    ToolCall {
        name: String,
        status: ToolCallStatus,
        duration_ms: Option<u128>,
        summary: String,
        collapsed: bool,
    },
    Evidence(String),
    Error(String),
}

impl MessageBlock {
    pub fn append_assistant_delta(&mut self, delta: &str) -> bool {
        if let Self::AssistantText(message) = self {
            message.push_str(delta);
            return true;
        }
        false
    }

    pub fn render_lines(&self) -> Vec<Line<'static>> {
        match self {
            Self::User(message) => render_text_block("user", message, user_style()),
            Self::AssistantText(message) => {
                render_text_block("assistant", message, assistant_style())
            }
            Self::Thinking(message) => vec![Line::from(vec![
                Span::styled("thinking: ", dim_style()),
                Span::raw(message.clone()),
            ])],
            Self::ToolCall {
                name,
                status,
                duration_ms,
                summary,
                collapsed,
            } => render_tool_call(name, status, *duration_ms, summary, *collapsed),
            Self::Evidence(summary) => render_text_block("evidence", summary, evidence_style()),
            Self::Error(message) => render_text_block("error", message, error_style()),
        }
    }
}

pub fn blocks_to_text(blocks: &[MessageBlock], gap: bool) -> Text<'static> {
    let mut lines = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        if gap && index > 0 {
            lines.push(Line::from(""));
        }
        lines.extend(block.render_lines());
    }
    Text::from(lines)
}

fn render_text_block(label: &str, message: &str, style: Style) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(format!("{label}:"), style)));
    let mut in_code = false;
    for line in message.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_code = !in_code;
            lines.push(Line::from(Span::styled(
                format!("  | {line}"),
                code_style(),
            )));
        } else if in_code {
            lines.push(Line::from(Span::styled(
                format!("  | {line}"),
                code_style(),
            )));
        } else if looks_like_citation(trimmed) {
            lines.push(Line::from(Span::styled(
                format!("  {line}"),
                evidence_style(),
            )));
        } else {
            lines.push(Line::from(Span::raw(format!("  {line}"))));
        }
    }
    if message.is_empty() {
        lines.push(Line::from(""));
    }
    lines
}

fn render_tool_call(
    name: &str,
    status: &ToolCallStatus,
    duration_ms: Option<u128>,
    summary: &str,
    collapsed: bool,
) -> Vec<Line<'static>> {
    let status_text = match status {
        ToolCallStatus::Running => "running",
        ToolCallStatus::Succeeded => "ok",
        ToolCallStatus::Failed => "failed",
    };
    let duration = duration_ms
        .map(|value| format!(" duration_ms={value}"))
        .unwrap_or_default();
    let mut lines = vec![Line::from(vec![
        Span::styled("tool: ", tool_style()),
        Span::styled(name.to_string(), tool_style().add_modifier(Modifier::BOLD)),
        Span::raw(format!(" status={status_text}{duration}")),
    ])];
    if collapsed {
        if !summary.is_empty() {
            lines[0].spans.push(Span::raw(format!(" {summary}")));
        }
        return lines;
    }
    if !summary.is_empty() {
        lines.push(Line::from(Span::raw(format!("  {summary}"))));
    }
    lines
}

fn looks_like_citation(line: &str) -> bool {
    line.starts_with("- [")
        || line.starts_with("[")
        || line.contains("doc_id=")
        || line.contains("source:")
        || line.contains("url=")
}

fn user_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

fn assistant_style() -> Style {
    Style::default().fg(Color::Green)
}

fn tool_style() -> Style {
    Style::default().fg(Color::Yellow)
}

fn evidence_style() -> Style {
    Style::default().fg(Color::Blue)
}

fn error_style() -> Style {
    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
}

fn dim_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn code_style() -> Style {
    Style::default().fg(Color::Magenta)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assistant_delta_appends_to_assistant_block() {
        let mut block = MessageBlock::AssistantText("hello ".to_string());
        assert!(block.append_assistant_delta("world"));
        assert_eq!(
            block,
            MessageBlock::AssistantText("hello world".to_string())
        );
    }

    #[test]
    fn renders_code_and_citation_lines() {
        let block = MessageBlock::AssistantText(
            "text\n```rust\nlet x = 1;\n```\ndoc_id=page:1".to_string(),
        );
        let rendered = block.render_lines();
        let text = rendered
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        assert!(text.contains(&"  | ```rust".to_string()));
        assert!(text.contains(&"  | let x = 1;".to_string()));
        assert!(text.contains(&"  doc_id=page:1".to_string()));
    }
}
