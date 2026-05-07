use ratatui::style::{Color, Modifier, Style};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeMode {
    Dark,
    Light,
    Mono,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThemeTokens {
    pub user: Color,
    pub assistant: Color,
    pub tool: Color,
    pub evidence: Color,
    pub error: Color,
    pub warning: Color,
    pub dim: Color,
    pub accent: Color,
    pub status: Color,
    pub code: Color,
}

pub fn supported_modes() -> &'static [ThemeMode] {
    &[ThemeMode::Dark, ThemeMode::Light, ThemeMode::Mono]
}

pub fn tokens(mode: ThemeMode) -> ThemeTokens {
    match mode {
        ThemeMode::Dark => ThemeTokens {
            user: Color::Cyan,
            assistant: Color::Green,
            tool: Color::Yellow,
            evidence: Color::Blue,
            error: Color::Red,
            warning: Color::LightYellow,
            dim: Color::DarkGray,
            accent: Color::Magenta,
            status: Color::Green,
            code: Color::LightMagenta,
        },
        ThemeMode::Light => ThemeTokens {
            user: Color::Blue,
            assistant: Color::Green,
            tool: Color::Rgb(150, 90, 0),
            evidence: Color::Rgb(35, 95, 160),
            error: Color::Red,
            warning: Color::Rgb(170, 95, 0),
            dim: Color::Gray,
            accent: Color::Magenta,
            status: Color::Green,
            code: Color::Rgb(120, 40, 130),
        },
        ThemeMode::Mono => ThemeTokens {
            user: Color::White,
            assistant: Color::White,
            tool: Color::White,
            evidence: Color::White,
            error: Color::White,
            warning: Color::White,
            dim: Color::Gray,
            accent: Color::White,
            status: Color::White,
            code: Color::White,
        },
    }
}

fn current() -> ThemeTokens {
    let _ = supported_modes();
    tokens(ThemeMode::Dark)
}

pub fn title_style() -> Style {
    Style::default()
        .fg(current().accent)
        .add_modifier(Modifier::BOLD)
}

pub fn active_style() -> Style {
    Style::default().fg(current().warning)
}

pub fn status_style() -> Style {
    Style::default().fg(current().status)
}

pub fn user_style() -> Style {
    Style::default()
        .fg(current().user)
        .add_modifier(Modifier::BOLD)
}

pub fn assistant_style() -> Style {
    Style::default().fg(current().assistant)
}

pub fn tool_style() -> Style {
    Style::default().fg(current().tool)
}

pub fn evidence_style() -> Style {
    Style::default().fg(current().evidence)
}

pub fn error_style() -> Style {
    Style::default()
        .fg(current().error)
        .add_modifier(Modifier::BOLD)
}

pub fn warning_style() -> Style {
    Style::default().fg(current().warning)
}

pub fn dim_style() -> Style {
    Style::default().fg(current().dim)
}

pub fn code_style() -> Style {
    Style::default().fg(current().code)
}

pub fn help_style() -> Style {
    dim_style()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_dark_light_and_mono_tokens() {
        assert_ne!(tokens(ThemeMode::Dark), tokens(ThemeMode::Light));
        assert_eq!(tokens(ThemeMode::Mono).user, Color::White);
        assert_eq!(tokens(ThemeMode::Dark).error, Color::Red);
    }
}
