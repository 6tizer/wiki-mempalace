//! 修复动作模型：J6 Fixer 工作流的核心类型定义。
//!
//! 每个 finding（lint 或 gap）经映射后产生一个 FixAction，
//! 描述"应该做什么修复"以及"修复方式（自动/草稿/人工）"。

use serde::{Deserialize, Serialize};

/// 修复执行方式：决定该修复能否自动落地、还是需要人工确认。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FixActionType {
    /// 可安全自动执行，无需人工确认
    Auto,
    /// 生成草稿供人确认，确认后可自动应用
    Draft,
    /// 需人工处理，仅输出建议，系统不自动修改
    Manual,
}

/// 具体修复补丁：仅 Auto 类型可携带实际变更数据。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FixPatch {
    /// 补缺失段落骨架（空内容占位）
    AppendSections { sections: Vec<String> },
    /// 设置页面标题
    SetTitle { title: String },
    /// 补交叉引用
    AddXref { entity_label: String },
}

/// 修复动作：从 finding 到可执行修复的映射结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixAction {
    /// finding code，如 "page.incomplete"
    pub code: String,
    /// 修复执行方式
    pub fix_type: FixActionType,
    /// 人可读的修复描述
    pub description: String,
    /// 修复目标（page id / claim id / entity id）
    pub subject: Option<String>,
    /// 人可读标签
    pub subject_label: Option<String>,
    /// 自动修复的具体变更数据（仅 Auto 类型有）
    pub patch: Option<FixPatch>,
}
