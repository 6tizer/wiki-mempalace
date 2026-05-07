use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use super::theme;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolCallStatus {
    Running,
    Succeeded,
    Failed,
    Retrying,
}

pub fn render_tool_call(
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
        ToolCallStatus::Retrying => "retrying",
    };
    let marker = match status {
        ToolCallStatus::Running => "*",
        ToolCallStatus::Succeeded => "ok",
        ToolCallStatus::Failed => "!",
        ToolCallStatus::Retrying => "~",
    };
    let duration = duration_ms
        .map(|value| format!(" duration_ms={value}"))
        .unwrap_or_default();
    let mut lines = vec![Line::from(vec![
        Span::styled(format!("{marker} tool: "), status_style(status)),
        Span::styled(
            name.to_string(),
            status_style(status).add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" status={status_text}{duration}")),
    ])];
    if collapsed {
        if !summary.is_empty() {
            lines[0].spans.push(Span::raw(format!(" {summary}")));
        }
        return lines;
    }
    if summary.is_empty() {
        lines.push(Line::from(Span::styled("  no summary", dim_style())));
    } else {
        for line in summary.lines() {
            lines.push(Line::from(Span::raw(format!("  {line}"))));
        }
    }
    lines
}

fn status_style(status: &ToolCallStatus) -> Style {
    match status {
        ToolCallStatus::Running => theme::tool_style(),
        ToolCallStatus::Succeeded => theme::assistant_style(),
        ToolCallStatus::Failed => theme::error_style(),
        ToolCallStatus::Retrying => theme::warning_style(),
    }
}

fn dim_style() -> Style {
    theme::dim_style()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_collapsed_success_and_expanded_failure() {
        let ok = render_tool_call(
            "wiki_query",
            &ToolCallStatus::Succeeded,
            Some(7),
            "internal_results=2",
            true,
        );
        assert_eq!(
            line_text(&ok[0]),
            "ok tool: wiki_query status=ok duration_ms=7 internal_results=2"
        );

        let failed = render_tool_call(
            "web_search",
            &ToolCallStatus::Failed,
            Some(9),
            "provider unavailable",
            false,
        );
        assert_eq!(
            line_text(&failed[0]),
            "! tool: web_search status=failed duration_ms=9"
        );
        assert_eq!(line_text(&failed[1]), "  provider unavailable");
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }
}
