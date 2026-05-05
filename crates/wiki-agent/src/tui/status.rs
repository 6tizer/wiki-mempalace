#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TuiStatus {
    Ready,
    Thinking,
    Error(String),
}

impl TuiStatus {
    pub fn render(&self) -> String {
        match self {
            Self::Ready => "ready".to_string(),
            Self::Thinking => "thinking".to_string(),
            Self::Error(err) => format!("error: {err}"),
        }
    }
}
