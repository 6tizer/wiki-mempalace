use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::{OverlayPanel, TuiAction, TuiApp};

pub fn handle_key(app: &mut TuiApp, key: KeyEvent) -> TuiAction {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => TuiAction::Quit,
        KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.toggle_last_tool_collapse();
            TuiAction::None
        }
        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.toggle_overlay(OverlayPanel::Sessions);
            TuiAction::None
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.toggle_overlay(OverlayPanel::Plan);
            TuiAction::None
        }
        KeyCode::Char('?') => {
            app.toggle_overlay(OverlayPanel::Help);
            TuiAction::None
        }
        KeyCode::Esc => {
            if app.clear_overlay() || app.cancel_thinking() {
                TuiAction::None
            } else if app.input.is_empty() {
                TuiAction::Quit
            } else {
                app.input.clear();
                TuiAction::None
            }
        }
        KeyCode::Enter => app.submit_input(),
        KeyCode::Backspace => {
            app.input.pop();
            TuiAction::None
        }
        KeyCode::Tab => {
            app.toggle_panel();
            TuiAction::None
        }
        KeyCode::Up => {
            app.history_prev();
            TuiAction::None
        }
        KeyCode::Down => {
            app.history_next();
            TuiAction::None
        }
        KeyCode::PageUp => {
            app.scroll_up();
            TuiAction::None
        }
        KeyCode::PageDown => {
            app.scroll_down();
            TuiAction::None
        }
        KeyCode::Char(ch) => {
            app.input.push(ch);
            TuiAction::None
        }
        _ => TuiAction::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn typing_enter_and_tab_work() {
        let mut app = TuiApp::new("agent_manager");
        assert_eq!(
            handle_key(&mut app, key(KeyCode::Char('h'))),
            TuiAction::None
        );
        assert_eq!(
            handle_key(&mut app, key(KeyCode::Char('i'))),
            TuiAction::None
        );
        assert_eq!(handle_key(&mut app, key(KeyCode::Tab)), TuiAction::None);
        assert_eq!(
            handle_key(&mut app, key(KeyCode::Enter)),
            TuiAction::Submit("hi".into())
        );
    }

    #[test]
    fn page_keys_scroll_active_panel() {
        let mut app = TuiApp::new("agent_manager");
        assert_eq!(
            handle_key(&mut app, key(KeyCode::PageDown)),
            TuiAction::None
        );
        assert_eq!(app.conversation_scroll, 1);
        assert_eq!(handle_key(&mut app, key(KeyCode::PageUp)), TuiAction::None);
        assert_eq!(app.conversation_scroll, 0);
    }

    #[test]
    fn esc_clears_input_then_quits() {
        let mut app = TuiApp::new("agent_manager");
        app.input = "draft".into();
        assert_eq!(handle_key(&mut app, key(KeyCode::Esc)), TuiAction::None);
        assert!(app.input.is_empty());
        assert_eq!(handle_key(&mut app, key(KeyCode::Esc)), TuiAction::Quit);
    }

    #[test]
    fn ctrl_c_quits() {
        let mut app = TuiApp::new("agent_manager");
        let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(handle_key(&mut app, event), TuiAction::Quit);
    }

    #[test]
    fn ctrl_t_toggles_last_tool_card() {
        let mut app = TuiApp::new("agent_manager");
        app.push_events(&[crate::events::ChatEvent::ToolFinished {
            name: "wiki_query".to_string(),
            duration_ms: 1,
            summary: "internal_results=1".to_string(),
        }]);
        let event = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL);
        assert_eq!(handle_key(&mut app, event), TuiAction::None);
        assert!(app.activity.iter().any(|block| matches!(
            block,
            super::super::message::MessageBlock::ToolCall {
                name,
                collapsed: false,
                ..
            } if name == "wiki_query"
        )));
    }

    #[test]
    fn ctrl_r_ctrl_p_question_and_esc_manage_overlays() {
        let mut app = TuiApp::new("agent_manager");
        assert_eq!(
            handle_key(
                &mut app,
                KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL)
            ),
            TuiAction::None
        );
        assert_eq!(app.overlay, Some(OverlayPanel::Sessions));
        assert_eq!(
            handle_key(
                &mut app,
                KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL)
            ),
            TuiAction::None
        );
        assert_eq!(app.overlay, Some(OverlayPanel::Plan));
        assert_eq!(
            handle_key(&mut app, key(KeyCode::Char('?'))),
            TuiAction::None
        );
        assert_eq!(app.overlay, Some(OverlayPanel::Help));
        assert_eq!(handle_key(&mut app, key(KeyCode::Esc)), TuiAction::None);
        assert_eq!(app.overlay, None);
    }
}
