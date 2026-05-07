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
            Constraint::Length(1),
        ])
        .split(frame.area());

    if frame.area().width < 80 {
        frame.render_widget(panels::conversation(app), vertical[0]);
    } else {
        let main = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
            .split(vertical[0]);
        frame.render_widget(panels::conversation(app), main[0]);
        frame.render_widget(panels::activity(app), main[1]);
    }
    frame.render_widget(panels::input(app), vertical[1]);
    frame.render_widget(panels::help(), vertical[2]);
    frame.render_widget(panels::status(app), vertical[3]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::OverlayPanel;
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

    #[test]
    fn wide_layout_snapshot_shows_overlay_activity_and_footer() {
        let backend = TestBackend::new(220, 18);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let mut app = TuiApp::new("agent_manager");
        app.session_id = "s".to_string();
        app.set_runtime_labels("shared:wiki", "auto", "native");
        app.toggle_overlay(OverlayPanel::Help);
        terminal.draw(|frame| draw(frame, &app)).expect("draw");

        let screen = buffer_text(terminal.backend().buffer(), 220, 18);
        assert!(screen.contains("Help"));
        assert!(screen.contains("Ctrl-R session picker"));
        assert!(screen.contains("profile=agent_manager"));
        assert!(screen.contains("tools=0"));
    }

    #[test]
    fn narrow_layout_snapshot_hides_activity_panel() {
        let backend = TestBackend::new(60, 14);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let mut app = TuiApp::new("agent_manager");
        app.push_activity("tool: wiki_query started");
        app.push_user("hello");
        terminal.draw(|frame| draw(frame, &app)).expect("draw");

        let screen = buffer_text(terminal.backend().buffer(), 60, 14);
        assert!(screen.contains("Conversation"));
        assert!(screen.contains("user:"));
        assert!(!screen.contains("Activity"));
        assert!(!screen.contains("tool: wiki_query started"));
    }

    fn buffer_text(buffer: &ratatui::buffer::Buffer, width: u16, height: u16) -> String {
        let mut out = String::new();
        for y in 0..height {
            for x in 0..width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }
}
