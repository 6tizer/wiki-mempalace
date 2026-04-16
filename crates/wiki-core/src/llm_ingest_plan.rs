//! LLM 结构化 ingest 输出（JSON），由 `wiki-cli ingest-llm` 解析后写入引擎。

use crate::model::MemoryTier;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmIngestPlanV1 {
    pub version: u32,
    #[serde(default)]
    pub summary_title: String,
    #[serde(default)]
    pub summary_markdown: String,
    #[serde(default)]
    pub claims: Vec<LlmClaimDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmClaimDraft {
    pub text: String,
    /// `working` | `episodic` | `semantic` | `procedural`
    pub tier: String,
}

pub fn parse_memory_tier(s: &str) -> Result<MemoryTier, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "working" => Ok(MemoryTier::Working),
        "episodic" => Ok(MemoryTier::Episodic),
        "semantic" => Ok(MemoryTier::Semantic),
        "procedural" => Ok(MemoryTier::Procedural),
        x => Err(format!("unknown memory tier: {x}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_inline_json() {
        let j = r###"{
            "version": 1,
            "summary_title": "Note",
            "summary_markdown": "## TL;DR\nok",
            "claims": [{"text":"Redis is used","tier":"semantic"}]
        }"###;
        let p: LlmIngestPlanV1 = serde_json::from_str(j).unwrap();
        assert_eq!(p.version, 1);
        assert_eq!(p.claims.len(), 1);
        assert!(parse_memory_tier(&p.claims[0].tier).is_ok());
    }

    #[test]
    fn parses_fixture_file() {
        let j = include_str!("../../../tests/fixtures/ingest_llm_ok.json");
        let p: LlmIngestPlanV1 = serde_json::from_str(j.trim()).unwrap();
        assert_eq!(p.version, 1);
        assert!(!p.summary_markdown.is_empty());
        assert!(!p.claims.is_empty());
    }
}
