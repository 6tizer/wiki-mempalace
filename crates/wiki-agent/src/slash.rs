#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SlashCommand {
    Help,
    Tools,
    Profile(Option<String>),
    Web(Option<String>),
    Memory,
    Sessions,
    Exit,
    Unknown(String),
}

pub fn parse(input: &str) -> Option<SlashCommand> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return None;
    }
    let mut parts = trimmed.split_whitespace();
    let command = parts.next().unwrap_or_default();
    let rest = parts.next().map(ToString::to_string);
    Some(match command {
        "/help" => SlashCommand::Help,
        "/tools" => SlashCommand::Tools,
        "/profile" => SlashCommand::Profile(rest),
        "/web" => SlashCommand::Web(rest),
        "/memory" => SlashCommand::Memory,
        "/sessions" => SlashCommand::Sessions,
        "/exit" | "/quit" => SlashCommand::Exit,
        other => SlashCommand::Unknown(other.to_string()),
    })
}

pub fn help_text() -> &'static str {
    "Commands: /help, /tools, /profile [name], /web [auto|always|off], /memory, /sessions, /exit"
}
