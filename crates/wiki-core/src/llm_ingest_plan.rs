//! LLM 结构化 ingest 输出（JSON），由 `wiki-cli ingest-llm` 解析后写入引擎。

use crate::model::MemoryTier;
use serde::{Deserialize, Deserializer, Serialize};

/// 兼容两种 claims 格式：对象数组 或 纯字符串数组
fn deserialize_claims_flexible<'de, D>(deserializer: D) -> Result<Vec<LlmClaimDraft>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ClaimOrString {
        Obj(LlmClaimDraft),
        Str(String),
    }

    let items: Vec<ClaimOrString> = Vec::deserialize(deserializer)?;
    Ok(items
        .into_iter()
        .map(|item| match item {
            ClaimOrString::Obj(d) => d,
            ClaimOrString::Str(s) => LlmClaimDraft {
                text: s,
                tier: "semantic".to_string(),
            },
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmIngestPlanV1 {
    pub version: u32,
    #[serde(default)]
    pub summary_title: String,
    #[serde(default)]
    pub summary_markdown: String,
    #[serde(default, deserialize_with = "deserialize_claims_flexible")]
    pub claims: Vec<LlmClaimDraft>,
    #[serde(default)]
    pub entities: Vec<LlmEntityDraft>,
    #[serde(default)]
    pub relationships: Vec<LlmRelationDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmClaimDraft {
    pub text: String,
    /// `working` | `episodic` | `semantic` | `procedural`
    pub tier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmEntityDraft {
    pub label: String,
    /// `person` | `project` | `library` | `concept` | `file_path` | `decision` | `other`
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRelationDraft {
    pub from_label: String,
    /// `uses` | `depends_on` | `contradicts` | `caused` | `fixed` | `supersedes` | `related`
    pub relation: String,
    pub to_label: String,
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

    #[test]
    fn parses_claims_as_strings() {
        // LLM 有时返回纯字符串数组而非对象数组
        let j = r###"{
            "version": 1,
            "summary_title": "Test",
            "summary_markdown": "body",
            "claims": ["claim one", "claim two"]
        }"###;
        let p: LlmIngestPlanV1 = serde_json::from_str(j).unwrap();
        assert_eq!(p.claims.len(), 2);
        assert_eq!(p.claims[0].text, "claim one");
        assert_eq!(p.claims[0].tier, "semantic");
        assert_eq!(p.claims[1].text, "claim two");
    }

    #[test]
    fn parses_claims_mixed() {
        // 混合格式：对象 + 字符串
        let j = r###"{
            "version": 1,
            "claims": [{"text":"obj claim","tier":"working"}, "str claim"]
        }"###;
        let p: LlmIngestPlanV1 = serde_json::from_str(j).unwrap();
        assert_eq!(p.claims.len(), 2);
        assert_eq!(p.claims[0].text, "obj claim");
        assert_eq!(p.claims[0].tier, "working");
        assert_eq!(p.claims[1].text, "str claim");
        assert_eq!(p.claims[1].tier, "semantic");
    }
}
