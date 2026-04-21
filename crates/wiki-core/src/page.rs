//! Wiki 页：人类可读 Markdown；图结构与之并行而非替代。

use crate::model::{PageId, Scope};
use crate::schema::EntryType;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiPage {
    pub id: PageId,
    pub title: String,
    pub markdown: String,
    pub scope: Scope,
    pub updated_at: OffsetDateTime,
    pub outbound_page_titles: Vec<String>,
    /// 可选的条目类型：用于 lint 的完整度检查与晋升规则路由。
    /// 为 `None` 时表示该页不参与结构化生命周期（历史页面也能无损反序列化）。
    #[serde(default)]
    pub entry_type: Option<EntryType>,
}

impl WikiPage {
    pub fn new(title: impl Into<String>, markdown: impl Into<String>, scope: Scope) -> Self {
        let now = OffsetDateTime::now_utc();
        Self {
            id: PageId(Uuid::new_v4()),
            title: title.into(),
            markdown: markdown.into(),
            scope,
            updated_at: now,
            outbound_page_titles: Vec::new(),
            entry_type: None,
        }
    }

    /// Builder：显式绑定条目类型，让该页参与 lint 的完整度检查。
    pub fn with_entry_type(mut self, entry_type: EntryType) -> Self {
        self.entry_type = Some(entry_type);
        self
    }

    /// 从 markdown 中提取 `[[Page Title]]` 形式的 wikilink，并写入 `outbound_page_titles`。
    pub fn refresh_outbound_links(&mut self) {
        self.outbound_page_titles = extract_wikilinks(&self.markdown);
    }
}

/// 解析 `[[...]]` 语法，返回去重且保持首次出现顺序的标题。
pub fn extract_wikilinks(markdown: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = markdown.as_bytes();
    let mut i = 0usize;
    while i + 3 < bytes.len() {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            let start = i + 2;
            let mut j = start;
            while j + 1 < bytes.len() {
                if bytes[j] == b']' && bytes[j + 1] == b']' {
                    let t = markdown[start..j].trim();
                    if !t.is_empty() && !out.iter().any(|x| x == t) {
                        out.push(t.to_string());
                    }
                    i = j + 2;
                    break;
                }
                j += 1;
            }
            if j + 1 >= bytes.len() {
                break;
            }
            continue;
        }
        i += 1;
    }
    out
}

/// 提取 Markdown 中所有 ATX 风格的 heading 文本（1~6 级 `#`），去前后空白与尾部修饰 `#`。
///
/// 为什么不解析 `===` / `---` 的 Setext 风格：
/// 当前 wiki 投影全部用 `##` 写段落，Setext 很少出现；保持实现极简以避免误判。
pub fn extract_headings(markdown: &str) -> Vec<String> {
    let mut out = Vec::new();
    for raw in markdown.lines() {
        let line = raw.trim_start();
        if !line.starts_with('#') {
            continue;
        }
        let rest = line.trim_start_matches('#').trim_start();
        if rest.is_empty() {
            continue;
        }
        let cleaned = rest.trim_end_matches(|c: char| c == '#' || c.is_whitespace());
        if !cleaned.is_empty() {
            out.push(cleaned.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{extract_headings, extract_wikilinks};

    #[test]
    fn extracts_unique_wikilinks() {
        let md = "A [[One]] B [[Two]] [[One]]";
        let got = extract_wikilinks(md);
        assert_eq!(got, vec!["One".to_string(), "Two".to_string()]);
    }

    #[test]
    fn extracts_all_heading_levels() {
        let md = "# 顶层\n## 定义\n正文\n### 来源引用 ###\n不是标题\n";
        let got = extract_headings(md);
        assert_eq!(got, vec!["顶层", "定义", "来源引用"]);
    }

    #[test]
    fn ignores_hash_inside_paragraph() {
        let md = "段落里有 # 号不应被当成标题\n## 真正的标题";
        let got = extract_headings(md);
        assert_eq!(got, vec!["真正的标题"]);
    }
}
