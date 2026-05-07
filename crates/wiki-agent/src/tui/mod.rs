mod app;
mod input;
mod layout;
mod message;
mod panels;
mod status;
mod theme;

use std::{
    io::{self, IsTerminal, Stdout},
    time::Duration,
};

use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::Terminal;
use ratatui_crossterm::CrosstermBackend;

use crate::chat::{self, ChatOptions, ChatRuntime};

use self::{
    app::{TuiAction, TuiApp},
    status::TuiStatus,
};

type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

pub(crate) fn run(options: ChatOptions) -> Result<(), Box<dyn std::error::Error>> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        eprintln!("wiki-agent tui requires a TTY; falling back to CLI chat.");
        return chat::run(options);
    }

    let mut runtime = chat::init_runtime(&options)?;
    let mut app = TuiApp::new(runtime.profile());
    app.push_activity("backend=native runtime=wiki-agent");
    if let Some(prompt) = options.one_shot_prompt.clone() {
        submit_prompt(
            &mut app,
            &mut runtime,
            &prompt,
            options.fake_llm_response.clone(),
        );
    }

    let mut terminal = TerminalSession::enter()?;
    let loop_result = run_loop(
        terminal.terminal_mut(),
        &mut app,
        &mut runtime,
        options.fake_llm_response,
    );
    drop(terminal);
    runtime.curate_memory_on_close()?;
    loop_result
}

fn run_loop(
    terminal: &mut TuiTerminal,
    app: &mut TuiApp,
    runtime: &mut ChatRuntime,
    fake_response: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        terminal.draw(|frame| layout::draw(frame, app))?;
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        match input::handle_key(app, key) {
            TuiAction::None => {}
            TuiAction::Quit => break,
            TuiAction::Submit(prompt) => {
                submit_prompt(app, runtime, &prompt, fake_response.clone());
            }
        }
    }
    Ok(())
}

fn submit_prompt(
    app: &mut TuiApp,
    runtime: &mut ChatRuntime,
    prompt: &str,
    fake_response: Option<String>,
) {
    app.push_user(prompt);
    app.set_status(TuiStatus::Thinking);
    app.push_activity(format!("manager: prompt chars={}", prompt.chars().count()));
    match runtime.run_prompt(prompt, fake_response) {
        Ok(turn) => {
            app.push_events(&turn.events);
            app.push_evidence(turn.evidence);
            if turn.answer.is_empty() {
                app.push_assistant_message("");
            } else {
                for chunk in turn.answer.split_inclusive(' ') {
                    app.push_assistant_delta(chunk);
                }
            }
            app.set_status(TuiStatus::Ready);
        }
        Err(err) => {
            app.push_error(err.to_string());
            app.set_status(TuiStatus::Error(err.to_string()));
        }
    }
}

struct TerminalSession {
    terminal: TuiTerminal,
}

impl TerminalSession {
    fn enter() -> Result<Self, Box<dyn std::error::Error>> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(err) = execute!(stdout, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(Box::new(err));
        }
        let backend = CrosstermBackend::new(stdout);
        let terminal = match Terminal::new(backend) {
            Ok(terminal) => terminal,
            Err(err) => {
                let _ = disable_raw_mode();
                let mut stdout = io::stdout();
                let _ = execute!(stdout, LeaveAlternateScreen);
                return Err(Box::new(err));
            }
        };
        Ok(Self { terminal })
    }

    fn terminal_mut(&mut self) -> &mut TuiTerminal {
        &mut self.terminal
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}
