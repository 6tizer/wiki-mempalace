//! 消费链产物的最终标准化层。
//!
//! 所有消费入口在内部已生成 page 后，通过 `finalize_consumed_page`
//! 统一对齐 entry_type、status、段落骨架与 PageContract 标准。

use wiki_core::{Confidence, DomainSchema, EntryType, WikiPage};

/// 消费产物的最终标准化：确保 entry_type、status、段落骨架与 PageContract 标准对齐。
///
/// 用于 crystallize / ingest-llm 等"内部已 insert page"的入口——
/// 这些入口内部自己生成了 page，需要在最后一步统一对齐。
pub fn finalize_consumed_page(
    page: &mut WikiPage,
    entry_type: EntryType,
    _confidence: Confidence,
    schema: &DomainSchema,
) {
    // 1. 设置 entry_type
    page.entry_type = Some(entry_type.clone());

    // 2. 用 initial_status_for 计算并设置 status
    let status = crate::initial_status_for(Some(&entry_type), schema);
    if page.status != status {
        page.status = status;
        page.status_entered_at = Some(time::OffsetDateTime::now_utc());
    }

    // 3. 检查 markdown 是否包含骨架段落，缺失的追加到末尾
    let template = entry_type.section_template();
    for section_name in template {
        let heading = format!("## {}", section_name);
        if !page.markdown.contains(&heading) {
            page.markdown.push_str(&format!("\n\n## {}\n\n（暂无）", section_name));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiki_core::model::Scope;
    use wiki_core::EntryStatus;

    fn test_schema() -> DomainSchema {
        DomainSchema::permissive_default()
    }

    fn test_scope() -> Scope {
        Scope::Private {
            agent_id: "test".into(),
        }
    }

    #[test]
    fn finalize_sets_entry_type() {
        let mut page = WikiPage::new("测试", "# 测试\n\n正文", test_scope());
        finalize_consumed_page(&mut page, EntryType::Summary, Confidence::default(), &test_schema());
        assert_eq!(page.entry_type, Some(EntryType::Summary));
    }

    #[test]
    fn finalize_sets_correct_status_auto_approved() {
        // Summary 属于 auto_approved_on_create → Approved
        let mut page = WikiPage::new("测试", "# 测试\n\n正文", test_scope());
        finalize_consumed_page(&mut page, EntryType::Summary, Confidence::default(), &test_schema());
        assert_eq!(page.status, EntryStatus::Approved);
    }

    #[test]
    fn finalize_sets_correct_status_concept() {
        // Concept 不属于 auto_approved_on_create，且无 lifecycle rule → Draft
        let mut page = WikiPage::new("测试", "# 测试\n\n正文", test_scope());
        finalize_consumed_page(&mut page, EntryType::Concept, Confidence::default(), &test_schema());
        assert_eq!(page.status, EntryStatus::Draft);
    }

    #[test]
    fn finalize_appends_missing_sections() {
        let mut page = WikiPage::new("测试", "# 测试\n\n正文", test_scope());
        finalize_consumed_page(&mut page, EntryType::Qa, Confidence::default(), &test_schema());
        assert!(page.markdown.contains("## 问题\n\n（暂无）"));
        assert!(page.markdown.contains("## 回答\n\n（暂无）"));
    }

    #[test]
    fn finalize_preserves_existing_content() {
        // markdown 中已包含部分骨架段落，不应被覆盖
        let md = "# 测试\n\n## 问题\n\n已有问题内容\n".to_string();
        let mut page = WikiPage::new("测试", md, test_scope());
        finalize_consumed_page(&mut page, EntryType::Qa, Confidence::default(), &test_schema());
        assert!(page.markdown.contains("## 问题\n\n已有问题内容"));
        // 但缺失的 "回答" 仍应被追加
        assert!(page.markdown.contains("## 回答\n\n（暂无）"));
    }

    #[test]
    fn finalize_updates_status_entered_at() {
        let mut page = WikiPage::new("测试", "# 测试\n\n正文", test_scope());
        let before = page.status_entered_at;
        // 默认新建为 Draft， finalize Summary 会改为 Approved
        finalize_consumed_page(&mut page, EntryType::Summary, Confidence::default(), &test_schema());
        let after = page.status_entered_at;
        assert!(after > before, "status 变化时应更新 status_entered_at");
    }
}
