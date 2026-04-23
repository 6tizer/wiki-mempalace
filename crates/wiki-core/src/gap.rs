//! Gap 发现：知识缺口检测的结构定义。
//!
//! Gap 与 Lint Finding 语义不同：
//! - Lint finding 表示"已有内容有质量问题"（坏了）
//! - Gap finding 表示"应该有但缺失的内容"（缺了）
//! 两者独立建模，后续 Fixer（J6）需要区分处理。

use serde::{Deserialize, Serialize};

/// Gap 严重程度：影响知识补全优先级的分级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapSeverity {
    /// 知识空白明显，建议尽快补全
    High,
    /// 有一定覆盖但不充分，建议后续补全
    Medium,
    /// 轻微不足，可按需补全
    Low,
}

/// 知识缺口发现结果。
///
/// 每条记录代表系统检测到的一处"应该有但缺失"的知识。
/// `code` 字段遵循 `gap.{type}` 命名约定，与 LintFinding 的 `{domain}.{check}` 风格对齐。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapFinding {
    /// 缺口类型编码，如 `gap.missing_xref`、`gap.low_coverage`、`gap.orphan_source`
    pub code: String,
    /// 人类可读的缺口描述
    pub message: String,
    /// 缺口严重程度
    pub severity: GapSeverity,
    /// 缺口关联的主体 ID（source / claim / entity 的 UUID 字符串）
    pub subject: Option<String>,
    /// 缺口关联的标题或标签（方便人类定位）
    pub subject_label: Option<String>,
}

