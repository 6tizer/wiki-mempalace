//! 质量与矛盾：lint 结果结构；矛盾裁决留给 LLM/人工，此处保留「提示」数据结构。

use crate::model::ClaimId;
use crate::page::{extract_headings, WikiPage};
use crate::schema::DomainSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LintSeverity {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintFinding {
    pub code: String,
    pub message: String,
    pub severity: LintSeverity,
    pub subject: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContradictionHint {
    pub a: ClaimId,
    pub b: ClaimId,
    pub reason: String,
}

/// 按 `CompletenessConfig` 的"lint 基线"检查页面是否缺少必需段落。
///
/// 约束来源：
/// - 仅当 `page.entry_type` 非空时才会检查（历史页面默认不参与）；
/// - 若 schema 中该 EntryType 的必需段落列表为空，则返回空结果；
/// - 段落匹配采用精确字符串匹配（trim 后比较），以避免中英混用误判。
pub fn check_page_completeness(schema: &DomainSchema, page: &WikiPage) -> Vec<LintFinding> {
    let Some(entry_type) = page.entry_type.as_ref() else {
        return Vec::new();
    };
    let required = schema.required_sections_for(entry_type);
    if required.is_empty() {
        return Vec::new();
    }

    let headings: HashSet<String> = extract_headings(&page.markdown)
        .into_iter()
        .map(|h| h.trim().to_string())
        .collect();

    let mut out = Vec::new();
    for section in required {
        let key = section.trim();
        if !headings.contains(key) {
            out.push(LintFinding {
                code: "page.incomplete".into(),
                message: format!("页面缺少必需段落：{key}"),
                severity: LintSeverity::Warn,
                subject: Some(page.id.0.to_string()),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Scope;
    use crate::schema::EntryType;

    fn page_with_md(md: &str, et: Option<EntryType>) -> WikiPage {
        let p = WikiPage::new(
            "T",
            md,
            Scope::Private {
                agent_id: "a".into(),
            },
        );
        if let Some(e) = et {
            p.with_entry_type(e)
        } else {
            p
        }
    }

    #[test]
    fn no_entry_type_means_no_findings() {
        let schema = DomainSchema::permissive_default();
        let p = page_with_md("无内容", None);
        assert!(check_page_completeness(&schema, &p).is_empty());
    }

    #[test]
    fn missing_required_sections_reported() {
        let schema = DomainSchema::permissive_default();
        let p = page_with_md("## 定义\n只写了定义段\n", Some(EntryType::Concept));
        let findings = check_page_completeness(&schema, &p);
        // concept 默认要求 3 段，只写了 1 段，应产生 2 条缺段报告
        let codes: Vec<_> = findings.iter().map(|f| f.code.as_str()).collect();
        assert_eq!(findings.len(), 2);
        assert!(codes.iter().all(|c| *c == "page.incomplete"));
    }

    #[test]
    fn all_required_sections_present_no_findings() {
        let schema = DomainSchema::permissive_default();
        let md = "## 定义\n\n## 关键要点\n\n## 来源引用\n";
        let p = page_with_md(md, Some(EntryType::Concept));
        assert!(check_page_completeness(&schema, &p).is_empty());
    }
}
