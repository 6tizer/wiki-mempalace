use ratatui::style::{Color, Modifier, Style};

pub fn title_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

pub fn active_style() -> Style {
    Style::default().fg(Color::Yellow)
}

pub fn status_style() -> Style {
    Style::default().fg(Color::Green)
}

pub fn error_style() -> Style {
    Style::default().fg(Color::Red)
}

pub fn help_style() -> Style {
    Style::default().fg(Color::DarkGray)
}
