//! 迁移过程中的中间数据模型。
//!
//! 设计思路：
//! - `RawPage` 从单个 .md 文件解析出来，尽量保留原始字段（未做语义转换）。
//! - `ResolvedPage` 是经过跨库引用解析后的最终产物，可以直接落盘成
//!   本地 wiki 的 markdown（带 YAML frontmatter）。
//! - 外部链接保留为 `RawLink`，resolve 阶段尝试与 Source URL 做 join。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 导出库的来源类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LibraryKind {
    /// 知识 Wiki DB
    Wiki,
    /// X 书签文章数据库
    XBookmark,
    /// 微信文章数据库
    WeChat,
}

impl LibraryKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Wiki => "wiki",
            Self::XBookmark => "x_bookmark",
            Self::WeChat => "wechat",
        }
    }
}

/// 正文里抽到的一个链接（未解析）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawLink {
    /// 锚文本
    pub text: String,
    /// 原始 href（相对路径 or 绝对 URL）
    pub href: String,
    /// 分类：Internal（指向另一个 .md 文件） / External（http(s) URL）
    pub kind: LinkKind,
    /// Internal 链接从文件名抽出的 Notion UUID（32 位 hex，无连字符）
    pub target_uuid: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkKind {
    Internal,
    External,
}

/// 从单个 .md 文件解析出来的原始记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPage {
    /// 所属库
    pub library: LibraryKind,
    /// 原始 .md 文件路径（便于 debug）
    pub source_path: PathBuf,
    /// 从文件名末尾提取的 32 位 hex UUID
    pub notion_uuid: String,
    /// 页面标题（从正文第一行 `# 标题` 提取）
    pub title: String,
    /// 属性块（`Key: value` 字面）
    pub properties: Vec<(String, String)>,
    /// 正文（去掉标题和属性块之后）
    pub body: String,
    /// 从正文提取的所有链接
    pub links: Vec<RawLink>,
}
