use crate::events::{activity_lines, ChatEvent};
use std::io::{self, Write};

pub fn render_assistant_message(message: &str) -> io::Result<()> {
    let mut stdout = io::stdout();
    for chunk in message.split_inclusive('\n') {
        write!(stdout, "{chunk}")?;
        stdout.flush()?;
    }
    if !message.ends_with('\n') {
        writeln!(stdout)?;
    }
    Ok(())
}

pub fn render_system_line(message: &str) {
    println!("{message}");
}

pub(crate) fn render_chat_events(events: &[ChatEvent]) {
    for line in activity_lines(events) {
        render_system_line(&line);
    }
}
