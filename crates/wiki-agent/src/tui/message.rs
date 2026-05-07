use ratatui::{
    style::Style,
    text::{Line, Span, Text},
};

use super::theme;
use super::tool_call::{render_tool_call, ToolCallStatus};

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
            Self::User(message) => render_text_block("user", message, theme::user_style()),
            Self::AssistantText(message) => {
                render_text_block("assistant", message, theme::assistant_style())
            }
            Self::Thinking(message) => vec![Line::from(vec![
                Span::styled("thinking: ", theme::dim_style()),
                Span::raw(message.clone()),
            ])],
            Self::ToolCall {
                name,
                status,
                duration_ms,
                summary,
                collapsed,
            } => render_tool_call(name, status, *duration_ms, summary, *collapsed),
            Self::Evidence(summary) => {
                render_text_block("evidence", summary, theme::evidence_style())
            }
            Self::Error(message) => render_text_block("error", message, theme::error_style()),
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
                theme::code_style(),
            )));
        } else if in_code {
            lines.push(Line::from(Span::styled(
                format!("  | {line}"),
                theme::code_style(),
            )));
        } else if looks_like_citation(trimmed) {
            lines.push(Line::from(Span::styled(
                format!("  {line}"),
                theme::evidence_style(),
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

fn looks_like_citation(line: &str) -> bool {
    line.starts_with("- [")
        || line.starts_with("[")
        || line.contains("doc_id=")
        || line.contains("source:")
        || line.contains("url=")
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

    #[test]
    fn renders_message_block_snapshot() {
        let blocks = vec![
            MessageBlock::User("hello".to_string()),
            MessageBlock::AssistantText("answer\nsource: wiki".to_string()),
            MessageBlock::Evidence("internal=1 web_status=off".to_string()),
            MessageBlock::ToolCall {
                name: "wiki_query".to_string(),
                status: ToolCallStatus::Succeeded,
                duration_ms: Some(5),
                summary: "internal_results=1".to_string(),
                collapsed: true,
            },
            MessageBlock::Error("failed".to_string()),
        ];

        let rendered = blocks_to_text(&blocks, true);
        let text = rendered.lines.iter().map(line_text).collect::<Vec<_>>();

        assert_eq!(
            text,
            vec![
                "user:",
                "  hello",
                "",
                "assistant:",
                "  answer",
                "  source: wiki",
                "",
                "evidence:",
                "  internal=1 web_status=off",
                "",
                "ok tool: wiki_query status=ok duration_ms=5 internal_results=1",
                "",
                "error:",
                "  failed",
            ]
        );
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }
}
