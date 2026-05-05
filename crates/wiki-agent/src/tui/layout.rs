use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

use super::{app::TuiApp, panels};

pub fn draw(frame: &mut Frame<'_>, app: &TuiApp) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
        .split(vertical[0]);

    frame.render_widget(panels::conversation(app), main[0]);
    frame.render_widget(panels::activity(app), main[1]);
    frame.render_widget(panels::input(app), vertical[1]);
    frame.render_widget(panels::status(app), vertical[2]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn draw_handles_small_and_wide_sizes() {
        for (width, height) in [(40, 12), (120, 32)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).expect("terminal");
            let mut app = TuiApp::new("agent_manager");
            app.push_user("hello");
            app.push_assistant_delta("world");
            app.push_activity("tool: wiki_query");
            terminal.draw(|frame| draw(frame, &app)).expect("draw");
        }
    }
}
