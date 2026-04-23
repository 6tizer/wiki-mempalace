//! 消费链产物的统一契约层。
//!
//! 所有消费入口生成 page 前必须构造 PageContract，
//! 以保证产出的 WikiPage 在 entry_type、段落骨架、frontmatter 格式上
//! 与 vault-standards.md 完全对齐。

use crate::{Confidence, EntryStatus, EntryType, Scope, WikiPage};
use std::collections::BTreeMap;

/// 消费链产物的统一契约。所有消费入口生成 page 前必须构造此结构。
///
/// 保证产出的 WikiPage 在 entry_type、段落骨架、frontmatter 格式上
/// 与 vault-standards.md 定义的标准完全对齐。
#[derive(Debug, Clone)]
pub struct PageContract {
    /// 产物标题（也是 H1 标题）
    pub title: String,
    /// 条目类型
    pub entry_type: EntryType,
    /// 置信度
    pub confidence: Confidence,
    /// 标签列表
    pub tags: Vec<String>,
    /// 来源入口标识（如 "query", "crystallize", "qa", "synthesis", "batch-ingest"）
    pub source: String,
    /// 段落名 → 正文内容的映射。按 entry_type.section_template() 顺序输出。
    /// 未提供的段落自动填"（暂无）"。
    pub sections: BTreeMap<String, String>,
    /// summary 特有：回填自 source 的 URL
    pub source_url: Option<String>,
    /// summary 特有：来自 source frontmatter 的标签
    pub source_tags: Vec<String>,
}

impl PageContract {
    /// 创建一个新的 PageContract，entry_type 和 title 必填。
    pub fn new(title: impl Into<String>, entry_type: EntryType) -> Self {
        Self {
            title: title.into(),
            entry_type,
            confidence: Confidence::default(),
            tags: Vec::new(),
            source: String::new(),
            sections: BTreeMap::new(),
            source_url: None,
            source_tags: Vec::new(),
        }
    }

    /// Builder：设置置信度
    pub fn with_confidence(mut self, confidence: Confidence) -> Self {
        self.confidence = confidence;
        self
    }

    /// Builder：设置标签
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Builder：设置来源入口
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    /// Builder：添加一个段落
    pub fn with_section(mut self, name: impl Into<String>, content: impl Into<String>) -> Self {
        self.sections.insert(name.into(), content.into());
        self
    }

    /// Builder：设置 source_url（summary 特有）
    pub fn with_source_url(mut self, url: impl Into<String>) -> Self {
        self.source_url = Some(url.into());
        self
    }

    /// Builder：设置 source_tags（summary 特有）
    pub fn with_source_tags(mut self, tags: Vec<String>) -> Self {
        self.source_tags = tags;
        self
    }

    /// 按骨架模板拼接标准 markdown。
    ///
    /// 格式：`# {title}\n\n## {section}\n\n{content}\n\n...`
    /// 按 entry_type.section_template() 的顺序输出段落；
    /// sections map 中有的取值，没有的填"（暂无）"。
    /// Index 类型无骨架，直接输出 sections 中的所有内容（按 key 排序）。
    pub fn render_markdown(&self) -> String {
        let mut md = format!("# {}\n\n", self.title);
        let template = self.entry_type.section_template();
        if template.is_empty() {
            // Index 类型：按 key 排序输出所有段落
            md.push_str(
                &self
                    .sections
                    .iter()
                    .map(|(name, content)| format!("## {}\n\n{}\n", name, content))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        } else {
            md.push_str(
                &template
                    .iter()
                    .map(|name| {
                        let content = self
                            .sections
                            .get(*name)
                            .map(|s| s.as_str())
                            .unwrap_or("（暂无）");
                        format!("## {}\n\n{}\n", name, content)
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        md
    }

    /// 生成标准 WikiPage。status 由调用方传入（因为 initial_status_for 在 wiki-kernel）。
    pub fn into_page(self, scope: Scope, status: EntryStatus) -> WikiPage {
        let md = self.render_markdown();
        WikiPage::new(self.title, md, scope)
            .with_entry_type(self.entry_type)
            .with_status(status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Scope;

    fn test_scope() -> Scope {
        Scope::Private {
            agent_id: "test".into(),
        }
    }

    #[test]
    fn section_template_concept_has_4_sections() {
        assert_eq!(EntryType::Concept.section_template().len(), 4);
    }

    #[test]
    fn section_template_qa_has_2_sections() {
        assert_eq!(EntryType::Qa.section_template().len(), 2);
    }

    #[test]
    fn section_template_summary_has_5_sections() {
        assert_eq!(EntryType::Summary.section_template().len(), 5);
    }

    #[test]
    fn section_template_index_is_empty() {
        assert!(EntryType::Index.section_template().is_empty());
    }

    #[test]
    fn page_contract_render_markdown_fills_missing_sections() {
        let contract =
            PageContract::new("测试", EntryType::Concept).with_section("定义", "这是一个定义");
        let md = contract.render_markdown();
        assert!(md.starts_with("# 测试\n\n"));
        assert!(md.contains("## 定义\n\n这是一个定义"));
        assert!(md.contains("## 关键要点\n\n（暂无）"));
        assert!(md.contains("## 本文语境\n\n（暂无）"));
        assert!(md.contains("## 来源引用\n\n（暂无）"));
    }

    #[test]
    fn page_contract_render_markdown_preserves_provided_sections() {
        let contract = PageContract::new("测试", EntryType::Qa)
            .with_section("问题", "这是什么？")
            .with_section("回答", "这是答案。");
        let md = contract.render_markdown();
        assert!(md.starts_with("# 测试\n\n"));
        assert!(md.contains("## 问题\n\n这是什么？"));
        assert!(md.contains("## 回答\n\n这是答案。"));
    }

    #[test]
    fn page_contract_render_markdown_respects_template_order() {
        let contract = PageContract::new("测试", EntryType::Concept)
            .with_section("来源引用", "引用内容")
            .with_section("定义", "定义内容")
            .with_section("本文语境", "语境内容")
            .with_section("关键要点", "要点内容");
        let md = contract.render_markdown();
        assert!(md.starts_with("# 测试\n\n"));
        let pos_def = md.find("## 定义").unwrap();
        let pos_key = md.find("## 关键要点").unwrap();
        let pos_ctx = md.find("## 本文语境").unwrap();
        let pos_src = md.find("## 来源引用").unwrap();
        assert!(pos_def < pos_key);
        assert!(pos_key < pos_ctx);
        assert!(pos_ctx < pos_src);
    }

    #[test]
    fn page_contract_into_page_sets_entry_type() {
        let contract = PageContract::new("测试", EntryType::Summary);
        let page = contract.into_page(test_scope(), EntryStatus::Draft);
        assert_eq!(page.entry_type, Some(EntryType::Summary));
    }

    #[test]
    fn page_contract_into_page_uses_given_status() {
        let contract = PageContract::new("测试", EntryType::Entity);
        let page = contract.into_page(test_scope(), EntryStatus::Approved);
        assert_eq!(page.status, EntryStatus::Approved);
    }

    #[test]
    fn page_contract_builder_pattern() {
        let contract = PageContract::new("测试", EntryType::Concept)
            .with_confidence(Confidence::High)
            .with_tags(vec!["tag1".into(), "tag2".into()])
            .with_source("query")
            .with_section("定义", "定义内容")
            .with_source_url("https://example.com")
            .with_source_tags(vec!["a".into()]);
        assert_eq!(contract.title, "测试");
        assert_eq!(contract.entry_type, EntryType::Concept);
        assert_eq!(contract.confidence, Confidence::High);
        assert_eq!(contract.tags, vec!["tag1", "tag2"]);
        assert_eq!(contract.source, "query");
        assert_eq!(
            contract.sections.get("定义"),
            Some(&"定义内容".to_string())
        );
        assert_eq!(contract.source_url, Some("https://example.com".to_string()));
        assert_eq!(contract.source_tags, vec!["a"]);
    }

    #[test]
    fn page_contract_index_type_outputs_all_sections() {
        let contract = PageContract::new("索引", EntryType::Index)
            .with_section("B段", "B内容")
            .with_section("A段", "A内容")
            .with_section("C段", "C内容");
        let md = contract.render_markdown();
        assert!(md.starts_with("# 索引\n\n"));
        let pos_a = md.find("## A段").unwrap();
        let pos_b = md.find("## B段").unwrap();
        let pos_c = md.find("## C段").unwrap();
        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
        assert!(md.contains("## A段\n\nA内容"));
        assert!(md.contains("## B段\n\nB内容"));
        assert!(md.contains("## C段\n\nC内容"));
    }
}
