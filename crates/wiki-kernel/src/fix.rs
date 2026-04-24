//! J6 Fixer 工作流：finding → fix action 映射层。
//!
//! 将 lint 和 gap 扫描产生的 finding 映射为可执行的修复动作，
//! 支持 Auto（自动执行）、Draft（草稿确认）、Manual（人工处理）三种类型。

use wiki_core::{FixAction, FixActionType, FixPatch, GapFinding, LintFinding};

/// 把 lint finding 映射为 fix action。
///
/// 根据 code 匹配对应的修复方式与描述，未知 code 默认按 Manual 处理。
pub fn map_lint_finding(finding: &LintFinding) -> FixAction {
    let code = finding.code.as_str();
    let subject = finding.subject.clone();

    match code {
        "page.incomplete" => {
            // 从消息中提取缺失段落名
            let section = extract_section_from_incomplete_message(&finding.message)
                .unwrap_or_else(|| "待补充段落".to_string());
            FixAction {
                code: finding.code.clone(),
                fix_type: FixActionType::Auto,
                description: format!("补充缺失段落骨架：{}", section),
                subject,
                subject_label: None,
                patch: Some(FixPatch::AppendSections {
                    sections: vec![section],
                }),
            }
        }
        "page.empty_title" => FixAction {
            code: finding.code.clone(),
            fix_type: FixActionType::Auto,
            description: "页面标题为空，从内容第一行提取标题".to_string(),
            subject: subject.clone(),
            subject_label: subject,
            // 映射层无法读取 page markdown，patch 在 apply_auto_fixes 层通过 fallback 生成
            patch: None,
        },
        "xref.missing" => FixAction {
            code: finding.code.clone(),
            fix_type: FixActionType::Draft,
            description: "建议补充交叉引用，将 claim 关键词关联到相关页面".to_string(),
            subject,
            subject_label: None,
            patch: None,
        },
        "page.broken_wikilink" => {
            let link_target = extract_broken_link(&finding.message);
            FixAction {
                code: finding.code.clone(),
                fix_type: FixActionType::Manual,
                description: format!("引用链接已断：{link_target}，需人工确认链接目标"),
                subject,
                subject_label: None,
                patch: None,
            }
        }
        "page.orphan" => FixAction {
            code: finding.code.clone(),
            fix_type: FixActionType::Manual,
            description: "页面无任何入站链接，需人工决定是否保留".to_string(),
            subject,
            subject_label: None,
            patch: None,
        },
        "claim.stale" => FixAction {
            code: finding.code.clone(),
            fix_type: FixActionType::Manual,
            description: "claim 已过期，需人工确认是否删除或取代".to_string(),
            subject,
            subject_label: None,
            patch: None,
        },
        "quality.low" => FixAction {
            code: finding.code.clone(),
            fix_type: FixActionType::Manual,
            description: "claim 质量分低于阈值，需人工提升质量".to_string(),
            subject,
            subject_label: None,
            patch: None,
        },
        "lifecycle.stale" => FixAction {
            code: finding.code.clone(),
            fix_type: FixActionType::Manual,
            description: "claim 生命周期已过期，需人工确认是否更新".to_string(),
            subject,
            subject_label: None,
            patch: None,
        },
        _ => FixAction {
            code: finding.code.clone(),
            fix_type: FixActionType::Manual,
            description: format!("未知 finding：{}，默认按人工处理", finding.code),
            subject,
            subject_label: None,
            patch: None,
        },
    }
}

/// 把 gap finding 映射为 fix action。
///
/// 未知 code 默认按 Manual 处理。
pub fn map_gap_finding(finding: &GapFinding) -> FixAction {
    let code = finding.code.as_str();
    let subject = finding.subject.clone();
    let subject_label = finding.subject_label.clone();

    match code {
        "gap.missing_xref" => FixAction {
            code: finding.code.clone(),
            fix_type: FixActionType::Draft,
            description: "建议补充交叉引用，将 claim 关键词关联到相关页面".to_string(),
            subject,
            subject_label,
            patch: None,
        },
        "gap.low_coverage" => FixAction {
            code: finding.code.clone(),
            fix_type: FixActionType::Manual,
            description: "entity 覆盖不足，需人工补充内容".to_string(),
            subject,
            subject_label,
            patch: None,
        },
        "gap.orphan_source" => FixAction {
            code: finding.code.clone(),
            fix_type: FixActionType::Manual,
            description: "source 孤立无引用，需人工决定消化或丢弃".to_string(),
            subject,
            subject_label,
            patch: None,
        },
        _ => FixAction {
            code: finding.code.clone(),
            fix_type: FixActionType::Manual,
            description: format!("未知 gap：{}，默认按人工处理", finding.code),
            subject,
            subject_label,
            patch: None,
        },
    }
}

/// 合并同一 subject 的 `page.incomplete` fixes，将多个 `AppendSections` 的段落列表聚合。
///
/// 减少重复写入，同时让 patch 更紧凑。
fn dedup_fixes(fixes: &mut Vec<FixAction>) {
    let mut append_map: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    // 先收集所有 page.incomplete 的段落，对同一 subject 的 section 去重
    for fix in fixes.iter() {
        if fix.code == "page.incomplete" && fix.fix_type == FixActionType::Auto {
            if let Some(ref subject) = fix.subject {
                if let Some(FixPatch::AppendSections { sections }) = &fix.patch {
                    let set: std::collections::HashSet<String> = sections.iter().cloned().collect();
                    append_map.entry(subject.clone()).or_default().extend(set);
                }
            }
        }
    }

    // 重建列表：同一 subject 的 page.incomplete 只保留一条，其余原样保留
    let mut seen = std::collections::HashSet::new();
    let mut merged = Vec::new();
    for fix in fixes.drain(..) {
        if fix.code == "page.incomplete" && fix.fix_type == FixActionType::Auto {
            if let Some(ref subject) = fix.subject {
                if seen.insert(subject.clone()) {
                    if let Some(sections) = append_map.get(subject) {
                        merged.push(FixAction {
                            code: fix.code,
                            fix_type: fix.fix_type,
                            description: format!("补充缺失段落骨架：{}", sections.join("、")),
                            subject: fix.subject,
                            subject_label: fix.subject_label,
                            patch: Some(FixPatch::AppendSections {
                                sections: sections.clone(),
                            }),
                        });
                        continue;
                    }
                } else {
                    continue;
                }
            }
        }
        merged.push(fix);
    }

    *fixes = merged;
}

/// 批量映射（方便 CLI 调用）。
///
/// 先处理 lint findings，再处理 gap findings，合并为单一结果列表。
/// 同一 page 的 `page.incomplete` 会被聚合为一条，段落列表合并。
pub fn map_findings_to_fixes(
    lint_findings: &[LintFinding],
    gap_findings: &[GapFinding],
) -> Vec<FixAction> {
    let mut fixes: Vec<FixAction> = lint_findings.iter().map(map_lint_finding).collect();
    fixes.extend(gap_findings.iter().map(map_gap_finding));
    dedup_fixes(&mut fixes);
    fixes
}

/// 从 "页面缺少必需段落：{section}" 中提取段落名。
///
/// 使用 `find` + 切片做健壮匹配，避免硬切固定位置；即使消息前后有额外内容也能提取。
/// 提取后截断换行后内容，只保留第一行。
fn extract_section_from_incomplete_message(msg: &str) -> Option<String> {
    let marker = "页面缺少必需段落：";
    msg.find(marker).and_then(|pos| {
        let start = pos + marker.len();
        let section = msg[start..].lines().next().unwrap_or(&msg[start..]).trim();
        if section.is_empty() {
            None
        } else {
            Some(section.to_string())
        }
    })
}

/// 从 "broken wikilink: {link}" 中提取链接目标。
fn extract_broken_link(msg: &str) -> String {
    msg.strip_prefix("broken wikilink: ")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| msg.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiki_core::{GapFinding, GapSeverity, LintFinding, LintSeverity};

    fn make_lint(code: &str, message: &str, subject: Option<&str>) -> LintFinding {
        LintFinding {
            code: code.into(),
            message: message.into(),
            severity: LintSeverity::Warn,
            subject: subject.map(|s| s.to_string()),
        }
    }

    fn make_gap(
        code: &str,
        message: &str,
        subject: Option<&str>,
        subject_label: Option<&str>,
    ) -> GapFinding {
        GapFinding {
            code: code.into(),
            message: message.into(),
            severity: GapSeverity::Medium,
            subject: subject.map(|s| s.to_string()),
            subject_label: subject_label.map(|s| s.to_string()),
        }
    }

    #[test]
    fn lint_page_incomplete_maps_to_auto_with_append_sections() {
        let finding = make_lint("page.incomplete", "页面缺少必需段落：定义", Some("page-1"));
        let fix = map_lint_finding(&finding);
        assert_eq!(fix.code, "page.incomplete");
        assert_eq!(fix.fix_type, FixActionType::Auto);
        assert!(!fix.description.is_empty());
        assert_eq!(fix.subject, Some("page-1".to_string()));
        match fix.patch {
            Some(FixPatch::AppendSections { sections }) => {
                assert_eq!(sections, vec!["定义"]);
            }
            other => panic!("期望 AppendSections，得到 {:?}", other),
        }
    }

    #[test]
    fn lint_page_incomplete_extracts_key_points_from_message() {
        // 验证从 "页面缺少必需段落：关键要点" 正确提取段落名
        let finding = make_lint(
            "page.incomplete",
            "页面缺少必需段落：关键要点",
            Some("page-1"),
        );
        let fix = map_lint_finding(&finding);
        assert_eq!(fix.code, "page.incomplete");
        match fix.patch {
            Some(FixPatch::AppendSections { sections }) => {
                assert_eq!(sections, vec!["关键要点"]);
            }
            other => panic!("期望 AppendSections，得到 {:?}", other),
        }
    }

    #[test]
    fn multiple_incomplete_findings_merge_into_single_patch_with_multiple_sections() {
        // 同一 page 的多个 page.incomplete 应合并为一条，patch 包含多个 section
        let f1 = make_lint("page.incomplete", "页面缺少必需段落：定义", Some("page-1"));
        let f2 = make_lint(
            "page.incomplete",
            "页面缺少必需段落：关键要点",
            Some("page-1"),
        );
        let f3 = make_lint(
            "page.incomplete",
            "页面缺少必需段落：来源引用",
            Some("page-1"),
        );
        let fixes = map_findings_to_fixes(&[f1, f2, f3], &[]);
        let incomplete_fixes: Vec<&FixAction> = fixes
            .iter()
            .filter(|f| f.code == "page.incomplete")
            .collect();
        assert_eq!(
            incomplete_fixes.len(),
            1,
            "同一 page 的 page.incomplete 应合并为一条"
        );
        match &incomplete_fixes[0].patch {
            Some(FixPatch::AppendSections { sections }) => {
                assert_eq!(sections, &vec!["定义", "关键要点", "来源引用"]);
            }
            other => panic!("期望 AppendSections，得到 {:?}", other),
        }
    }

    #[test]
    fn lint_page_empty_title_maps_to_auto_with_no_patch() {
        let finding = make_lint(
            "page.empty_title",
            "wiki page has empty title",
            Some("page-2"),
        );
        let fix = map_lint_finding(&finding);
        assert_eq!(fix.code, "page.empty_title");
        assert_eq!(fix.fix_type, FixActionType::Auto);
        assert!(!fix.description.is_empty());
        // 映射层不读取 page markdown，patch 在 apply_auto_fixes 层通过 fallback 生成
        assert!(fix.patch.is_none());
    }

    #[test]
    fn lint_xref_missing_maps_to_draft() {
        let finding = make_lint(
            "xref.missing",
            "claim keywords are not referenced in current pages",
            Some("claim-1"),
        );
        let fix = map_lint_finding(&finding);
        assert_eq!(fix.fix_type, FixActionType::Draft);
        assert!(fix.patch.is_none());
        assert!(!fix.description.is_empty());
    }

    #[test]
    fn lint_broken_wikilink_maps_to_manual() {
        let finding = make_lint(
            "page.broken_wikilink",
            "broken wikilink: 不存在的页面",
            Some("page-3"),
        );
        let fix = map_lint_finding(&finding);
        assert_eq!(fix.fix_type, FixActionType::Manual);
        assert!(fix.patch.is_none());
        assert!(fix.description.contains("不存在的页面"));
    }

    #[test]
    fn lint_page_orphan_maps_to_manual() {
        let finding = make_lint(
            "page.orphan",
            "page has no inbound wikilinks",
            Some("page-4"),
        );
        let fix = map_lint_finding(&finding);
        assert_eq!(fix.fix_type, FixActionType::Manual);
        assert!(!fix.description.is_empty());
    }

    #[test]
    fn lint_claim_stale_maps_to_manual() {
        let finding = make_lint(
            "claim.stale",
            "stale claim retained for audit",
            Some("claim-2"),
        );
        let fix = map_lint_finding(&finding);
        assert_eq!(fix.fix_type, FixActionType::Manual);
        assert!(!fix.description.is_empty());
    }

    #[test]
    fn lint_quality_low_maps_to_manual() {
        let finding = make_lint(
            "quality.low",
            "claim quality below threshold",
            Some("claim-3"),
        );
        let fix = map_lint_finding(&finding);
        assert_eq!(fix.fix_type, FixActionType::Manual);
        assert!(!fix.description.is_empty());
    }

    #[test]
    fn lint_lifecycle_stale_maps_to_manual() {
        let finding = make_lint(
            "lifecycle.stale",
            "stale claim retained for audit",
            Some("claim-4"),
        );
        let fix = map_lint_finding(&finding);
        assert_eq!(fix.fix_type, FixActionType::Manual);
        assert!(!fix.description.is_empty());
    }

    #[test]
    fn lint_unknown_code_fallback_to_manual() {
        let finding = make_lint("page.unknown_check", "something weird", Some("page-5"));
        let fix = map_lint_finding(&finding);
        assert_eq!(fix.fix_type, FixActionType::Manual);
        assert!(fix.description.contains("未知"));
    }

    #[test]
    fn gap_missing_xref_maps_to_draft() {
        let finding = make_gap(
            "gap.missing_xref",
            "claim keywords are not referenced",
            Some("claim-5"),
            Some("关键词测试"),
        );
        let fix = map_gap_finding(&finding);
        assert_eq!(fix.fix_type, FixActionType::Draft);
        assert!(fix.patch.is_none());
        assert_eq!(fix.subject_label, Some("关键词测试".to_string()));
        assert!(!fix.description.is_empty());
    }

    #[test]
    fn gap_low_coverage_maps_to_manual() {
        let finding = make_gap(
            "gap.low_coverage",
            "entity 'Redis' 仅有 1 条 claim，低于阈值 2",
            Some("entity-1"),
            Some("Redis"),
        );
        let fix = map_gap_finding(&finding);
        assert_eq!(fix.fix_type, FixActionType::Manual);
        assert!(!fix.description.is_empty());
    }

    #[test]
    fn gap_orphan_source_maps_to_manual() {
        let finding = make_gap(
            "gap.orphan_source",
            "source 孤立",
            Some("source-1"),
            Some("file:///notes/a.md"),
        );
        let fix = map_gap_finding(&finding);
        assert_eq!(fix.fix_type, FixActionType::Manual);
        assert!(!fix.description.is_empty());
    }

    #[test]
    fn gap_unknown_code_fallback_to_manual() {
        let finding = make_gap("gap.unknown", "unknown gap", Some("x-1"), Some("label"));
        let fix = map_gap_finding(&finding);
        assert_eq!(fix.fix_type, FixActionType::Manual);
        assert!(fix.description.contains("未知"));
    }

    #[test]
    fn batch_mapping_merges_both() {
        let lint = make_lint("page.orphan", "orphan", Some("page-6"));
        let gap = make_gap("gap.low_coverage", "low cov", Some("entity-2"), Some("E2"));
        let fixes = map_findings_to_fixes(&[lint], &[gap]);
        assert_eq!(fixes.len(), 2);
        assert_eq!(fixes[0].code, "page.orphan");
        assert_eq!(fixes[0].fix_type, FixActionType::Manual);
        assert_eq!(fixes[1].code, "gap.low_coverage");
        assert_eq!(fixes[1].fix_type, FixActionType::Manual);
    }

    #[test]
    fn batch_mapping_empty_input_returns_empty() {
        let fixes = map_findings_to_fixes(&[], &[]);
        assert!(fixes.is_empty());
    }
}
