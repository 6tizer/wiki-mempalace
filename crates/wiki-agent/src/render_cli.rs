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
