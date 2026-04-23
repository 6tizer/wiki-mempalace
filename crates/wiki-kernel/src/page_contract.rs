//! 消费链产物的最终标准化层。
//!
//! 所有消费入口在内部已生成 page 后，通过 `finalize_consumed_page`
//! 统一对齐 entry_type、status、段落骨架与 PageContract 标准。

use wiki_core::{Confidence, DomainSchema, EntryType, WikiPage};

/// 将 markdown 中的旧段落标题重命名为标准段落标题。
///
/// mapping 中每个元组为 `(旧标题完整字符串, 新标题完整字符串)`，
/// 可直接包含 `# ` 或 `## ` 前缀，按顺序做全局字符串替换。
fn remap_sections(markdown: &mut String, mapping: &[(&str, &str)]) {
    for (old, new) in mapping {
        *markdown = markdown.replace(old, new);
    }
}

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

    // 2. 根据类型做段落映射
    match &entry_type {
        EntryType::Synthesis => {
            remap_sections(&mut page.markdown, &[
                ("# 问题\n\n", "## 研究问题\n\n"),
                ("## 结论与发现", "## 综合分析"),
                ("## 涉及文件", "## 来源列表"),
                ("## 可复用教训", "## 关键发现"),
            ]);
        }
        EntryType::LintReport => {
            remap_sections(&mut page.markdown, &[
                ("# Gap Report\n\n", "## 总体健康度\n\n"),
                ("## high", "## 问题清单"),
                ("## medium", "## 建议"),
                ("## low", "## 检查日期"),
            ]);
        }
        _ => {}
    }

    // 3. 用 initial_status_for 计算并设置 status
    let status = crate::initial_status_for(Some(&entry_type), schema);
    if page.status != status {
        page.status = status;
        page.status_entered_at = Some(time::OffsetDateTime::now_utc());
    }

    // 4. 检查 markdown 是否包含骨架段落，缺失的追加到末尾
    let template = entry_type.section_template();
    for section_name in template {
        let heading = format!("## {}", section_name);
        if !page.markdown.contains(&heading) {
            page.markdown
                .push_str(&format!("\n\n## {}\n\n（暂无）", section_name));
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
        finalize_consumed_page(
            &mut page,
            EntryType::Summary,
            Confidence::default(),
            &test_schema(),
        );
        assert_eq!(page.entry_type, Some(EntryType::Summary));
    }

    #[test]
    fn finalize_sets_correct_status_auto_approved() {
        // Summary 属于 auto_approved_on_create → Approved
        let mut page = WikiPage::new("测试", "# 测试\n\n正文", test_scope());
        finalize_consumed_page(
            &mut page,
            EntryType::Summary,
            Confidence::default(),
            &test_schema(),
        );
        assert_eq!(page.status, EntryStatus::Approved);
    }

    #[test]
    fn finalize_sets_correct_status_concept() {
        // Concept 不属于 auto_approved_on_create，且无 lifecycle rule → Draft
        let mut page = WikiPage::new("测试", "# 测试\n\n正文", test_scope());
        finalize_consumed_page(
            &mut page,
            EntryType::Concept,
            Confidence::default(),
            &test_schema(),
        );
        assert_eq!(page.status, EntryStatus::Draft);
    }

    #[test]
    fn finalize_appends_missing_sections() {
        let mut page = WikiPage::new("测试", "# 测试\n\n正文", test_scope());
        finalize_consumed_page(
            &mut page,
            EntryType::Qa,
            Confidence::default(),
            &test_schema(),
        );
        assert!(page.markdown.contains("## 问题\n\n（暂无）"));
        assert!(page.markdown.contains("## 回答\n\n（暂无）"));
    }

    #[test]
    fn finalize_preserves_existing_content() {
        // markdown 中已包含部分骨架段落，不应被覆盖
        let md = "# 测试\n\n## 问题\n\n已有问题内容\n".to_string();
        let mut page = WikiPage::new("测试", md, test_scope());
        finalize_consumed_page(
            &mut page,
            EntryType::Qa,
            Confidence::default(),
            &test_schema(),
        );
        assert!(page.markdown.contains("## 问题\n\n已有问题内容"));
        // 但缺失的 "回答" 仍应被追加
        assert!(page.markdown.contains("## 回答\n\n（暂无）"));
    }

    #[test]
    fn finalize_updates_status_entered_at() {
        let mut page = WikiPage::new("测试", "# 测试\n\n正文", test_scope());
        let before = page.status_entered_at;
        // 默认新建为 Draft， finalize Summary 会改为 Approved
        finalize_consumed_page(
            &mut page,
            EntryType::Summary,
            Confidence::default(),
            &test_schema(),
        );
        let after = page.status_entered_at;
        assert!(after > before, "status 变化时应更新 status_entered_at");
    }

    #[test]
    fn finalize_remaps_crystallize_sections() {
        let md = "# 问题\n\n什么是 Rust？\n\n## 结论与发现\n\n- 发现 1\n\n## 涉及文件\n\n- `main.rs`\n\n## 可复用教训\n\n- 教训 1\n";
        let mut page = WikiPage::new("结晶测试", md.to_string(), test_scope());
        finalize_consumed_page(
            &mut page,
            EntryType::Synthesis,
            Confidence::default(),
            &test_schema(),
        );
        // 旧段落名应被重命名
        assert!(!page.markdown.contains("## 结论与发现"));
        assert!(!page.markdown.contains("## 涉及文件"));
        assert!(!page.markdown.contains("## 可复用教训"));
        // 新段落名应存在
        assert!(page.markdown.contains("## 研究问题"));
        assert!(page.markdown.contains("## 综合分析"));
        assert!(page.markdown.contains("## 来源列表"));
        assert!(page.markdown.contains("## 关键发现"));
        // 内容应保留
        assert!(page.markdown.contains("什么是 Rust？"));
        assert!(page.markdown.contains("发现 1"));
        assert!(page.markdown.contains("main.rs"));
        assert!(page.markdown.contains("教训 1"));
    }

    #[test]
    fn finalize_remaps_gap_sections() {
        let md = "# Gap Report\n\n- total gaps: `3`\n\n## high\n\n- `gap.coverage` 缺少覆盖\n\n## medium\n\n- `gap.orphan` 孤立页面\n\n## low\n\n- `gap.stale` 过期 claim\n";
        let mut page = WikiPage::new("gap 报告", md.to_string(), test_scope());
        finalize_consumed_page(
            &mut page,
            EntryType::LintReport,
            Confidence::default(),
            &test_schema(),
        );
        // 旧段落名应被重命名
        assert!(!page.markdown.contains("## high"));
        assert!(!page.markdown.contains("## medium"));
        assert!(!page.markdown.contains("## low"));
        // 新段落名应存在
        assert!(page.markdown.contains("## 总体健康度"));
        assert!(page.markdown.contains("## 问题清单"));
        assert!(page.markdown.contains("## 建议"));
        assert!(page.markdown.contains("## 检查日期"));
        // 内容应保留
        assert!(page.markdown.contains("total gaps: `3`"));
        assert!(page.markdown.contains("gap.coverage"));
    }

    #[test]
    fn finalize_appends_missing_sections_after_remap() {
        // 模拟 crystallize 只生成部分段落的情况
        let md = "# 问题\n\n什么是 Rust？\n\n## 结论与发现\n\n- 发现 1\n";
        let mut page = WikiPage::new("结晶测试", md.to_string(), test_scope());
        finalize_consumed_page(
            &mut page,
            EntryType::Synthesis,
            Confidence::default(),
            &test_schema(),
        );
        // 映射后已有的段落
        assert!(page.markdown.contains("## 研究问题"));
        assert!(page.markdown.contains("## 综合分析"));
        // 映射后缺失的段落应被补充
        assert!(page.markdown.contains("## 关键发现\n\n（暂无）"));
        assert!(page.markdown.contains("## 来源列表\n\n（暂无）"));
    }
}
