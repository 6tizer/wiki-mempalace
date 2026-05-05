use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum WorkerRole {
    Lint,
    Governance,
    Fixer,
    Synthesis,
    Search,
    MemoryCurator,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerAccess {
    Read,
    Write,
}

impl WorkerRole {
    pub fn agent_name(self) -> &'static str {
        match self {
            Self::Lint => "lint_agent",
            Self::Governance => "governance_agent",
            Self::Fixer => "fixer_agent",
            Self::Synthesis => "synthesis_agent",
            Self::Search => "search_agent",
            Self::MemoryCurator => "memory_curator",
        }
    }

    pub fn access(self, apply: bool) -> WorkerAccess {
        match self {
            Self::Fixer if apply => WorkerAccess::Write,
            _ => WorkerAccess::Read,
        }
    }

    pub fn mcp_tool_name(self) -> Option<&'static str> {
        match self {
            Self::Lint => Some("wiki_lint"),
            Self::Search => Some("wiki_query"),
            _ => None,
        }
    }
}
