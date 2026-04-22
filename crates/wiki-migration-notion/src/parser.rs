//! Notion Markdown 导出解析器。
//!
//! 三个库（Wiki / X书签 / 微信）的导出格式**完全同构**：
//!
//! ```text
//! # 标题
//!
//! Key: value     <- 属性块（Notion properties 的渲染）
//! Key: value
//!
//! <空行作为分隔>
//!
//! <Markdown 正文>
//! - [内部引用](相对路径%20{uuid32}.md)
//! - [外链](https://...)
//! ```
//!
//! 文件名末尾带 32 位 hex UUID（Notion 内部 page id），是跨库 join 的主 key。

use crate::model::{LibraryKind, LinkKind, RawLink, RawPage};
use anyhow::{Context, Result};
use percent_encoding::percent_decode_str;
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

/// 文件名尾部 32 位 hex UUID 正则：`... 09a31eaf99cc4161b51e7029278bc78e.md`
fn re_filename_uuid() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)\s([0-9a-f]{32})\.md$").unwrap())
}

/// Markdown 标准链接：`[text](href)`
///
/// 说明：我们不处理嵌套括号的病态情况，Notion 导出里没遇到。
fn re_markdown_link() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\[([^\]]*)\]\(([^)]+)\)").unwrap())
}

/// 在相对路径里抽末尾 32 位 hex UUID（`xxx%20{uuid}.md`）
fn re_href_uuid() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)([0-9a-f]{32})\.md$").unwrap())
}

/// 从文件名末尾提取 Notion UUID
pub fn extract_uuid_from_filename(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    re_filename_uuid()
        .captures(name)
        .map(|c| c[1].to_lowercase())
}

/// 解析单个 .md 文件
pub fn parse_file(path: &Path, library: LibraryKind) -> Result<RawPage> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("读取 {} 失败", path.display()))?;
    parse_content(path, library, &content)
}

/// 纯字符串版本（便于单测）
pub fn parse_content(path: &Path, library: LibraryKind, content: &str) -> Result<RawPage> {
    let notion_uuid = extract_uuid_from_filename(path)
        .with_context(|| format!("文件名里找不到 32 位 UUID: {}", path.display()))?;

    let mut lines = content.lines();

    // --- 第一行：# 标题 ---
    // 说明：Notion 导出保证第一行是 H1；若缺失则用文件名兜底。
    let title = lines
        .next()
        .map(|l| l.trim_start_matches('#').trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("untitled")
                .to_string()
        });

    // --- 属性块：连续的 `Key: value` 行，遇到空行或非 `Key: value` 停止 ---
    // 说明：Notion 属性值不会跨行（我们抽样里没见过），按单行 split 即可。
    let mut properties: Vec<(String, String)> = Vec::new();
    let mut body_lines: Vec<&str> = Vec::new();
    let mut in_header = true;

    for line in lines {
        if in_header {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                // 忽略 title 和属性块之间的空行；一旦出现第一个非属性的非空内容，转为 body
                continue;
            }
            // 只在形如 `Key: value` 且 Key 不含 Markdown 标记时视为属性
            if let Some((k, v)) = split_property(trimmed) {
                properties.push((k, v));
                continue;
            }
            in_header = false;
        }
        body_lines.push(line);
    }

    let body = body_lines.join("\n");

    // --- 抽链接 ---
    let links = extract_links(&body);

    Ok(RawPage {
        library,
        source_path: path.to_path_buf(),
        notion_uuid,
        title,
        properties,
        body,
        links,
    })
}

/// 判断一行是否是 `Key: value` 属性。
///
/// 规则（保守）：
/// - 必须有冒号（全角/半角皆可），且冒号左边不含空格/Markdown 标记（`-`, `*`, `#`, `[`, `|`）
/// - 冒号左边长度 ≤ 20 字符（中文属性键一般都很短，避免把正文句子当属性吞掉）
fn split_property(line: &str) -> Option<(String, String)> {
    let (sep_idx, sep_len) = if let Some(i) = line.find(": ") {
        (i, 2)
    } else if let Some(i) = line.find("：") {
        // 全角冒号 3 字节
        (i, "：".len())
    } else if line.ends_with(':') || line.ends_with('：') {
        return None;
    } else {
        return None;
    };

    let key = line[..sep_idx].trim();
    if key.is_empty() || key.chars().count() > 20 {
        return None;
    }
    // 禁用常见 Markdown 前缀
    if key.starts_with('-')
        || key.starts_with('*')
        || key.starts_with('#')
        || key.starts_with('[')
        || key.contains('|')
    {
        return None;
    }
    let value = line[sep_idx + sep_len..].trim();
    Some((key.to_string(), value.to_string()))
}

/// 从正文抽取所有 Markdown 链接
pub fn extract_links(body: &str) -> Vec<RawLink> {
    let mut out = Vec::new();
    for cap in re_markdown_link().captures_iter(body) {
        let text = cap[1].to_string();
        let href_raw = cap[2].trim().to_string();
        // 图片链接 `![](...)`：我们依然收集，后续决定要不要保留。
        // 但 Markdown 语法里 `![]` 前缀在链接 regex 匹配 `[...]` 时不会影响，
        // 因为图片语法是 `![alt](url)`——alt 在 `[]` 里，url 在 `()` 里，和普通 link 同形。
        // 不做特殊处理。

        let (kind, target_uuid) = classify_href(&href_raw);
        out.push(RawLink {
            text,
            href: href_raw,
            kind,
            target_uuid,
        });
    }
    out
}

fn classify_href(href: &str) -> (LinkKind, Option<String>) {
    // 绝对 URL
    if href.starts_with("http://") || href.starts_with("https://") {
        return (LinkKind::External, None);
    }
    // mailto / tel / 其它协议当外部处理（我们极少遇到）
    if href.contains("://") {
        return (LinkKind::External, None);
    }
    // 锚内跳转 `#section`：不视为任何边
    if href.starts_with('#') {
        return (LinkKind::External, None);
    }
    // 其余视为相对路径（指向另一个 Notion .md）
    let decoded = percent_decode_str(href).decode_utf8_lossy();
    let uuid = re_href_uuid()
        .captures(&decoded)
        .map(|c| c[1].to_lowercase());
    (LinkKind::Internal, uuid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn 从文件名提取_uuid() {
        let p = PathBuf::from("Gemma 4 09a31eaf99cc4161b51e7029278bc78e.md");
        assert_eq!(
            extract_uuid_from_filename(&p),
            Some("09a31eaf99cc4161b51e7029278bc78e".into())
        );
    }

    #[test]
    fn 文件名无_uuid_返回_none() {
        let p = PathBuf::from("随便.md");
        assert_eq!(extract_uuid_from_filename(&p), None);
    }

    #[test]
    fn 分离属性行_全角半角冒号皆可() {
        assert_eq!(
            split_property("作者: Alice"),
            Some(("作者".into(), "Alice".into()))
        );
        assert_eq!(
            split_property("作者：Bob"),
            Some(("作者".into(), "Bob".into()))
        );
    }

    #[test]
    fn 分离属性行_过滤_markdown_前缀() {
        assert_eq!(split_property("- key: value"), None);
        assert_eq!(split_property("## 标题: 文字"), None);
    }

    #[test]
    fn 抽链接_内部链接带_uuid() {
        let body = "See [Gemma 4](Gemma%204%2009a31eaf99cc4161b51e7029278bc78e.md) here.";
        let links = extract_links(body);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].kind, LinkKind::Internal);
        assert_eq!(
            links[0].target_uuid.as_deref(),
            Some("09a31eaf99cc4161b51e7029278bc78e")
        );
    }

    #[test]
    fn 抽链接_外部_url() {
        let body = "原文：[X推文](https://x.com/foo/status/123)";
        let links = extract_links(body);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].kind, LinkKind::External);
        assert_eq!(links[0].href, "https://x.com/foo/status/123");
        assert!(links[0].target_uuid.is_none());
    }

    #[test]
    fn 解析完整_md_样本() {
        let raw = "# 摘要：Gemma 4 测评\n\n作者: Alice\n类型: summary\n状态: 已审核\n\n## 一句话摘要\n\n[内部](X%20aaaabbbbccccddddeeeeffff00001111.md)\n链接：[https://x.com/foo](https://x.com/foo)\n";
        let path = PathBuf::from("Gemma 4 测评 1234567890abcdef1234567890abcdef.md");
        let page = parse_content(&path, LibraryKind::Wiki, raw).unwrap();
        assert_eq!(page.title, "摘要：Gemma 4 测评");
        assert_eq!(page.properties.len(), 3);
        assert_eq!(page.properties[0], ("作者".into(), "Alice".into()));
        assert_eq!(page.links.len(), 2);
        assert_eq!(page.links[0].kind, LinkKind::Internal);
        assert_eq!(page.links[1].kind, LinkKind::External);
    }
}
