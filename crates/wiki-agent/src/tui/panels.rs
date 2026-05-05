use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use super::app::{ActivePanel, TuiApp};
use super::theme;

pub fn conversation(app: &TuiApp) -> Paragraph<'_> {
    let title = if app.active_panel == ActivePanel::Conversation {
        "Conversation *"
    } else {
        "Conversation"
    };
    Paragraph::new(app.conversation.join("\n\n"))
        .block(
            Block::default()
                .title(title)
                .title_style(theme::title_style())
                .borders(Borders::ALL)
                .border_style(if app.active_panel == ActivePanel::Conversation {
                    theme::active_style()
                } else {
                    Default::default()
                }),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.conversation_scroll, 0))
}

pub fn activity(app: &TuiApp) -> Paragraph<'_> {
    let title = if app.active_panel == ActivePanel::Activity {
        "Activity *"
    } else {
        "Activity"
    };
    Paragraph::new(app.activity.join("\n"))
        .block(
            Block::default()
                .title(title)
                .title_style(theme::title_style())
                .borders(Borders::ALL)
                .border_style(if app.active_panel == ActivePanel::Activity {
                    theme::active_style()
                } else {
                    Default::default()
                }),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.activity_scroll, 0))
}

pub fn input(app: &TuiApp) -> Paragraph<'_> {
    Paragraph::new(app.input.as_str())
        .block(Block::default().title("Input").borders(Borders::ALL))
        .wrap(Wrap { trim: false })
}

pub fn status(app: &TuiApp) -> Paragraph<'_> {
    let style = if matches!(app.status, super::status::TuiStatus::Error(_)) {
        theme::error_style()
    } else {
        theme::status_style()
    };
    Paragraph::new(app.status_line()).style(style)
}
