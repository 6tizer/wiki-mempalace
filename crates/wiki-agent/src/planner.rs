use clap::ValueEnum;
use std::fmt;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum WebMode {
    Auto,
    Always,
    Off,
}

impl fmt::Display for WebMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => f.write_str("auto"),
            Self::Always => f.write_str("always"),
            Self::Off => f.write_str("off"),
        }
    }
}

impl FromStr for WebMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "always" => Ok(Self::Always),
            "off" => Ok(Self::Off),
            other => Err(format!("unknown web mode: {other}")),
        }
    }
}

pub fn should_use_web(query: &str, mode: WebMode) -> bool {
    match mode {
        WebMode::Off => false,
        WebMode::Always => true,
        WebMode::Auto => looks_external_or_fresh(query),
    }
}

pub fn private_scope_blocks_web(viewer_scope: &str, allow_private_web_search: bool) -> bool {
    !allow_private_web_search && viewer_scope.trim().starts_with("private:")
}

fn looks_external_or_fresh(query: &str) -> bool {
    let q = query.to_ascii_lowercase();
    let lower_hits = [
        "latest",
        "current",
        "today",
        "this week",
        "news",
        "release",
        "pricing",
    ];
    lower_hits.iter().any(|needle| q.contains(needle))
        || ["最新", "今天", "最近", "现在", "新闻", "价格", "发布"]
            .iter()
            .any(|needle| query.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_web_detects_freshness_queries() {
        assert!(should_use_web("latest xAI release", WebMode::Auto));
        assert!(should_use_web("最近有什么变化", WebMode::Auto));
        assert!(!should_use_web("explain my local wiki", WebMode::Auto));
    }

    #[test]
    fn private_scope_blocks_by_default() {
        assert!(private_scope_blocks_web("private:cli", false));
        assert!(!private_scope_blocks_web("private:cli", true));
        assert!(!private_scope_blocks_web("shared:wiki", false));
    }
}
