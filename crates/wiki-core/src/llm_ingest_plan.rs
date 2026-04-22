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
    /// 一句话摘要（vault `## 一句话摘要`；优先于 legacy `summary_markdown` 单独成段）
    #[serde(default)]
    pub one_sentence_summary: String,
    /// 关键洞察列表（vault `## 关键洞察` 以列表呈现）
    #[serde(default)]
    pub key_insights: Vec<String>,
    /// 对整篇 summary 的置信度：`high` | `medium` | `low`
    #[serde(default)]
    pub confidence: String,
    /// 建议写入 summary frontmatter 的 wiki 标签
    #[serde(default)]
    pub tags: Vec<String>,
    /// 从正文识别的作者（可选）
    #[serde(default)]
    pub source_author: Option<String>,
    /// 来源平台 / 出版方（可选）
    #[serde(default)]
    pub source_publisher: Option<String>,
    /// 原文发布时间（可选，自然语言或 ISO 字符串均可）
    #[serde(default)]
    pub source_published_at: Option<String>,
    #[serde(default, deserialize_with = "deserialize_claims_flexible")]
    pub claims: Vec<LlmClaimDraft>,
    #[serde(default)]
    pub entities: Vec<LlmEntityDraft>,
    #[serde(default)]
    pub relationships: Vec<LlmRelationDraft>,
}

impl LlmIngestPlanV1 {
    /// 将 LLM 填写的 `confidence` 规范为 `high` | `medium` | `low`。
    pub fn normalized_summary_confidence(&self) -> &'static str {
        match self.confidence.trim().to_ascii_lowercase().as_str() {
            "high" => "high",
            "low" => "low",
            "medium" | "med" | "" => "medium",
            // 未知取值保守记为 medium，避免破坏 YAML 枚举约定
            _ => "medium",
        }
    }

    /// 是否需要在引擎中物化 summary 页（含 vault 五段正文）。
    pub fn should_materialize_summary_page(&self) -> bool {
        !self.one_sentence_summary.trim().is_empty()
            || !self.key_insights.is_empty()
            || !self.summary_markdown.trim().is_empty()
            || !self.claims.is_empty()
    }

    /// 生成 vault 约定的 summary **正文**（5 个 `##` 段落，不含一级标题）。
    ///
    /// `footnote_url`：原始文章链接；batch 场景传 source 的 `url` 或 `file://` URI。
    pub fn to_five_section_summary_body(&self, footnote_url: Option<&str>) -> String {
        let legacy_only =
            self.one_sentence_summary.trim().is_empty() && !self.summary_markdown.trim().is_empty();

        let one_sentence = if !self.one_sentence_summary.trim().is_empty() {
            self.one_sentence_summary.trim().to_string()
        } else if !self.summary_markdown.trim().is_empty() {
            self.summary_markdown.trim().to_string()
        } else {
            "（暂无）".to_string()
        };

        let key_insights_block = if !self.key_insights.is_empty() {
            self.key_insights
                .iter()
                .map(|s| format!("- {}", s.trim()))
                .collect::<Vec<_>>()
                .join("\n")
        } else if !self.summary_markdown.trim().is_empty() && !legacy_only {
            // 新 schema：一句话已单独字段时，legacy 正文放到「关键洞察」
            self.summary_markdown.trim().to_string()
        } else {
            "（暂无）".to_string()
        };

        let concepts_block = if !self.claims.is_empty() {
            self.claims
                .iter()
                .map(|c| format!("- {}", c.text.trim()))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            "（暂无）".to_string()
        };

        let mut article_lines: Vec<String> = Vec::new();
        if let Some(url) = footnote_url {
            let u = url.trim();
            if !u.is_empty() {
                article_lines.push(format!("- 链接：`{u}`"));
            }
        }
        if let Some(ref a) = self.source_author {
            let a = a.trim();
            if !a.is_empty() {
                article_lines.push(format!("- 作者：{a}"));
            }
        }
        if let Some(ref p) = self.source_publisher {
            let p = p.trim();
            if !p.is_empty() {
                article_lines.push(format!("- 平台：{p}"));
            }
        }
        if let Some(ref t) = self.source_published_at {
            let t = t.trim();
            if !t.is_empty() {
                article_lines.push(format!("- 发布时间：{t}"));
            }
        }
        let article_block = if article_lines.is_empty() {
            "（暂无）".to_string()
        } else {
            article_lines.join("\n")
        };

        format!(
            "## 一句话摘要\n\n\
             {one_sentence}\n\n\
             ## 关键洞察\n\n\
             {key_insights_block}\n\n\
             ## 提取的概念\n\n\
             {concepts_block}\n\n\
             ## 原始文章信息\n\n\
             {article_block}\n\n\
             ## 个人评注\n\n\
             （暂无）\n"
        )
    }
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

    #[test]
    fn parses_extended_schema_with_defaults() {
        // 旧 JSON 无新字段时应等价于默认空值
        let j = r###"{"version":1,"summary_title":"x","summary_markdown":"y","claims":[]}"###;
        let p: LlmIngestPlanV1 = serde_json::from_str(j).unwrap();
        assert!(p.one_sentence_summary.is_empty());
        assert!(p.key_insights.is_empty());
        assert_eq!(p.normalized_summary_confidence(), "medium");
        assert!(p.tags.is_empty());
        assert!(p.source_author.is_none());
    }

    #[test]
    fn five_section_body_includes_all_headings() {
        let p = LlmIngestPlanV1 {
            version: 1,
            summary_title: "t".into(),
            summary_markdown: String::new(),
            one_sentence_summary: "一句".into(),
            key_insights: vec!["a".into(), "b".into()],
            confidence: "high".into(),
            tags: vec![],
            source_author: Some("作者".into()),
            source_publisher: None,
            source_published_at: Some("2020".into()),
            claims: vec![LlmClaimDraft {
                text: "概念一".into(),
                tier: "semantic".into(),
            }],
            entities: vec![],
            relationships: vec![],
        };
        let body = p.to_five_section_summary_body(Some("https://ex.test"));
        assert!(body.contains("## 一句话摘要"));
        assert!(body.contains("## 关键洞察"));
        assert!(body.contains("## 提取的概念"));
        assert!(body.contains("## 原始文章信息"));
        assert!(body.contains("## 个人评注"));
        assert!(body.contains("https://ex.test"));
        assert!(body.contains("概念一"));
    }
}
