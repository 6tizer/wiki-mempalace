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

    let items: Vec<ClaimOrString> =
        Option::<Vec<ClaimOrString>>::deserialize(deserializer)?.unwrap_or_default();
    Ok(items
        .into_iter()
        .map(|item| match item {
            ClaimOrString::Obj(d) => d,
            ClaimOrString::Str(s) => LlmClaimDraft {
                text: s,
                tier: "semantic".to_string(),
                tags: Vec::new(),
            },
        })
        .collect())
}

fn deserialize_string_or_default<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

fn deserialize_vec_or_default<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmIngestPlanV1 {
    pub version: u32,
    /// Production compiler richer summary contract. Legacy top-level summary
    /// fields below stay valid for existing callers.
    #[serde(default)]
    pub summary: LlmSummaryDraft,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub summary_title: String,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub summary_markdown: String,
    /// 一句话摘要（vault `## 一句话摘要`；优先于 legacy `summary_markdown` 单独成段）
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub one_sentence_summary: String,
    /// 关键洞察列表（vault `## 关键洞察` 以列表呈现）
    #[serde(default, deserialize_with = "deserialize_vec_or_default")]
    pub key_insights: Vec<String>,
    /// 对整篇 summary 的置信度：`high` | `medium` | `low`
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub confidence: String,
    /// 建议写入 summary frontmatter 的 wiki 标签
    #[serde(default, deserialize_with = "deserialize_vec_or_default")]
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
    pub concepts: Vec<LlmConceptDraft>,
    #[serde(default)]
    pub entities: Vec<LlmEntityDraft>,
    #[serde(default)]
    pub relationships: Vec<LlmRelationDraft>,
}

impl LlmIngestPlanV1 {
    /// 将 LLM 填写的 `confidence` 规范为 `high` | `medium` | `low`。
    pub fn normalized_summary_confidence(&self) -> &'static str {
        let confidence = if self.confidence.trim().is_empty() {
            self.summary.confidence.trim()
        } else {
            self.confidence.trim()
        };
        match confidence.to_ascii_lowercase().as_str() {
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
            || self.summary.has_content()
            || !self.key_insights.is_empty()
            || !self.summary_markdown.trim().is_empty()
            || !self.claims.is_empty()
            || !self.concepts.is_empty()
            || !self.entities.is_empty()
    }

    /// 生成 vault 约定的 summary **正文**（5 个 `##` 段落，不含一级标题）。
    ///
    /// `footnote_url`：原始文章链接；batch 场景传 source 的 `url` 或 `file://` URI。
    pub fn to_five_section_summary_body(&self, footnote_url: Option<&str>) -> String {
        let legacy_only =
            self.one_sentence_summary.trim().is_empty() && !self.summary_markdown.trim().is_empty();

        let one_sentence = if !self.one_sentence_summary.trim().is_empty() {
            self.one_sentence_summary.trim().to_string()
        } else if !self.summary.one_sentence_summary.trim().is_empty() {
            self.summary.one_sentence_summary.trim().to_string()
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
        } else if !self.summary.key_insights.is_empty() {
            self.summary
                .key_insights
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

        let concepts_block = {
            let mut lines = Vec::new();
            lines.extend(self.concepts.iter().map(|c| {
                let name = c.canonical_name.trim();
                let definition = c.definition.trim();
                if definition.is_empty() {
                    format!("- [[{name}]]")
                } else {
                    format!("- [[{name}]]：{definition}")
                }
            }));
            lines.extend(self.entities.iter().map(|e| {
                let name = e.canonical_or_label().trim().to_string();
                let profile = e.profile_or_definition().trim().to_string();
                if profile.is_empty() {
                    format!("- [[{name}]]")
                } else {
                    format!("- [[{name}]]：{profile}")
                }
            }));
            lines.extend(self.claims.iter().map(|c| format!("- {}", c.text.trim())));
            if lines.is_empty() {
                "（暂无）".to_string()
            } else {
                lines.join("\n")
            }
        };

        let mut article_lines: Vec<String> = Vec::new();
        if let Some(url) = footnote_url {
            let u = url.trim();
            if !u.is_empty() {
                article_lines.push(format!("- 链接：`{u}`"));
            }
        }
        let source_author = self
            .source_author
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or(self.summary.source_author.as_deref());
        let source_publisher = self
            .source_publisher
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or(self.summary.source_publisher.as_deref());
        let source_published_at = self
            .source_published_at
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or(self.summary.source_published_at.as_deref());
        if let Some(a) = source_author {
            let a = a.trim();
            if !a.is_empty() {
                article_lines.push(format!("- 作者：{a}"));
            }
        }
        if let Some(p) = source_publisher {
            let p = p.trim();
            if !p.is_empty() {
                article_lines.push(format!("- 平台：{p}"));
            }
        }
        if let Some(t) = source_published_at {
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LlmSummaryDraft {
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub title: String,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub one_sentence_summary: String,
    #[serde(default, deserialize_with = "deserialize_vec_or_default")]
    pub key_insights: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub confidence: String,
    #[serde(default, deserialize_with = "deserialize_vec_or_default")]
    pub tags: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub personal_note: String,
    #[serde(default)]
    pub source_author: Option<String>,
    #[serde(default)]
    pub source_publisher: Option<String>,
    #[serde(default)]
    pub source_published_at: Option<String>,
    #[serde(default)]
    pub external_url: Option<String>,
}

impl LlmSummaryDraft {
    pub fn has_content(&self) -> bool {
        !self.title.trim().is_empty()
            || !self.one_sentence_summary.trim().is_empty()
            || !self.key_insights.is_empty()
            || !self.personal_note.trim().is_empty()
            || !self.tags.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmClaimDraft {
    pub text: String,
    /// `working` | `episodic` | `semantic` | `procedural`
    pub tier: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LlmConceptDraft {
    #[serde(
        default,
        alias = "name",
        alias = "title",
        deserialize_with = "deserialize_string_or_default"
    )]
    pub canonical_name: String,
    /// Always `concept` for the production compiler contract.
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub kind: String,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub definition: String,
    #[serde(default, deserialize_with = "deserialize_vec_or_default")]
    pub key_points: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_vec_or_default")]
    pub tags: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_vec_or_default")]
    pub related_names: Vec<String>,
    #[serde(default)]
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LlmEntityDraft {
    /// Legacy field consumed by current CLI runner.
    pub label: String,
    /// `person` | `project` | `library` | `concept` | `file_path` | `decision` | `other`
    pub kind: String,
    /// Production compiler canonical page title.
    pub canonical_name: String,
    pub category: Option<String>,
    pub definition: String,
    pub profile: String,
    pub key_points: Vec<String>,
    pub tags: Vec<String>,
    pub related_names: Vec<String>,
}

impl<'de> Deserialize<'de> for LlmEntityDraft {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Default, Deserialize)]
        struct Raw {
            #[serde(default, deserialize_with = "deserialize_string_or_default")]
            label: String,
            #[serde(default, deserialize_with = "deserialize_string_or_default")]
            kind: String,
            #[serde(default, deserialize_with = "deserialize_string_or_default")]
            canonical_name: String,
            #[serde(default)]
            category: Option<String>,
            #[serde(default, deserialize_with = "deserialize_string_or_default")]
            definition: String,
            #[serde(default, deserialize_with = "deserialize_string_or_default")]
            profile: String,
            #[serde(default, deserialize_with = "deserialize_vec_or_default")]
            key_points: Vec<String>,
            #[serde(default, deserialize_with = "deserialize_vec_or_default")]
            tags: Vec<String>,
            #[serde(default, deserialize_with = "deserialize_vec_or_default")]
            related_names: Vec<String>,
        }

        let raw = Raw::deserialize(deserializer)?;
        let canonical_name = if raw.canonical_name.trim().is_empty() {
            raw.label.clone()
        } else {
            raw.canonical_name
        };
        let label = if raw.label.trim().is_empty() {
            canonical_name.clone()
        } else {
            raw.label
        };
        let kind = if raw.kind.trim().is_empty() {
            "other".to_string()
        } else {
            raw.kind
        };

        Ok(Self {
            label,
            kind,
            canonical_name,
            category: raw.category,
            definition: raw.definition,
            profile: raw.profile,
            key_points: raw.key_points,
            tags: raw.tags,
            related_names: raw.related_names,
        })
    }
}

impl LlmEntityDraft {
    pub fn canonical_or_label(&self) -> &str {
        if self.canonical_name.trim().is_empty() {
            &self.label
        } else {
            &self.canonical_name
        }
    }

    pub fn profile_or_definition(&self) -> &str {
        if self.profile.trim().is_empty() {
            &self.definition
        } else {
            &self.profile
        }
    }
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
    fn parses_rich_production_compiler_fixture() {
        let j = include_str!("../../../tests/fixtures/ingest_llm_plan_rich.json");
        let p: LlmIngestPlanV1 = serde_json::from_str(j.trim()).unwrap();
        assert_eq!(p.version, 1);
        assert_eq!(p.summary.title, "Production Wiki Compiler");
        assert_eq!(p.normalized_summary_confidence(), "high");
        assert_eq!(p.concepts.len(), 1);
        assert_eq!(p.concepts[0].canonical_name, "Production Wiki Compiler");
        assert_eq!(
            p.concepts[0].definition,
            "把原始 source 编译成 summary、concept、entity 页的本地流程。"
        );
        assert_eq!(p.concepts[0].key_points.len(), 2);
        assert_eq!(p.concepts[0].tags, vec!["wiki-compiler", "projection"]);
        assert_eq!(p.concepts[0].related_names, vec!["LlmIngestPlanV1"]);
        assert_eq!(p.entities.len(), 1);
        assert_eq!(p.entities[0].label, "Obsidian");
        assert_eq!(p.entities[0].canonical_name, "Obsidian");
        assert_eq!(p.entities[0].category.as_deref(), Some("tool"));
        assert_eq!(
            p.entities[0].profile,
            "用于浏览生成后 wiki graph 的本地知识库界面。"
        );
        assert_eq!(
            p.entities[0].key_points,
            vec!["可见 summary 和 concept/entity 链接"]
        );
        assert_eq!(p.entities[0].tags, vec!["obsidian", "wiki-ui"]);
        assert_eq!(
            p.entities[0].related_names,
            vec!["Production Wiki Compiler"]
        );
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
        assert!(p.claims[0].tags.is_empty());
        assert_eq!(p.claims[1].text, "claim two");
        assert!(p.claims[1].tags.is_empty());
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
        assert!(p.claims[1].tags.is_empty());
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
    fn parses_compiler_null_string_fields_as_empty_defaults() {
        let j = r###"{
            "version": 1,
            "summary": {
                "title": "t",
                "personal_note": null,
                "tags": null
            },
            "summary_title": "t",
            "summary_markdown": null,
            "one_sentence_summary": null,
            "key_insights": null,
            "confidence": null,
            "tags": null,
            "claims": null,
            "concepts": [
                {
                    "canonical_name": "API定价策略",
                    "kind": "concept",
                    "definition": null,
                    "key_points": null,
                    "tags": null,
                    "related_names": null,
                    "category": null
                }
            ],
            "entities": [
                {
                    "label": "DeepSeek-V4-Pro",
                    "kind": "other",
                    "canonical_name": "DeepSeek-V4-Pro",
                    "category": "model",
                    "definition": "模型 API 服务",
                    "profile": null,
                    "key_points": null,
                    "tags": null,
                    "related_names": null
                }
            ],
            "relationships": []
        }"###;

        let p: LlmIngestPlanV1 = serde_json::from_str(j).unwrap();

        assert!(p.summary_markdown.is_empty());
        assert!(p.claims.is_empty());
        assert!(p.concepts[0].definition.is_empty());
        assert!(p.entities[0].profile.is_empty());
        assert!(p.entities[0].key_points.is_empty());
    }

    #[test]
    fn claim_draft_old_json_without_tags_deserializes_to_empty_vec() {
        let j = r###"{"text":"claim","tier":"semantic"}"###;
        let claim: LlmClaimDraft = serde_json::from_str(j).unwrap();
        assert!(claim.tags.is_empty());
    }

    #[test]
    fn five_section_body_includes_all_headings() {
        let p = LlmIngestPlanV1 {
            version: 1,
            summary: LlmSummaryDraft::default(),
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
                tags: vec![],
            }],
            concepts: vec![],
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

    #[test]
    fn rich_summary_body_lists_concepts_and_entities() {
        let p: LlmIngestPlanV1 = serde_json::from_str(
            r###"{
                "version": 1,
                "summary": {
                    "one_sentence_summary": "本地编译器生成可见 wiki 页面。",
                    "key_insights": ["summary 与 concept/entity 分离"],
                    "source_author": "tester"
                },
                "concepts": [{
                    "canonical_name": "Wiki Compiler",
                    "definition": "source 到 wiki 页面集合的编译 contract"
                }],
                "entities": [{
                    "canonical_name": "Obsidian",
                    "category": "tool",
                    "profile": "本地 graph 浏览工具"
                }]
            }"###,
        )
        .unwrap();
        assert!(p.should_materialize_summary_page());
        assert_eq!(p.entities[0].label, "Obsidian");
        assert_eq!(p.entities[0].kind, "other");
        let body = p.to_five_section_summary_body(None);
        assert!(body.contains("本地编译器生成可见 wiki 页面。"));
        assert!(body.contains("- summary 与 concept/entity 分离"));
        assert!(body.contains("- [[Wiki Compiler]]：source 到 wiki 页面集合的编译 contract"));
        assert!(body.contains("- [[Obsidian]]：本地 graph 浏览工具"));
        assert!(body.contains("- 作者：tester"));
    }
}
