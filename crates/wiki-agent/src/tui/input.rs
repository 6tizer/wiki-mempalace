use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::{TuiAction, TuiApp};

pub fn handle_key(app: &mut TuiApp, key: KeyEvent) -> TuiAction {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => TuiAction::Quit,
        KeyCode::Esc => {
            if app.input.is_empty() {
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
}
