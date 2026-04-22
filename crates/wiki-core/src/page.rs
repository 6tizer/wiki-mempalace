//! Wiki 页：人类可读 Markdown；图结构与之并行而非替代。

use crate::model::{PageId, Scope};
use crate::schema::{EntryStatus, EntryType};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// `status` 字段的 serde default：历史 JSON 无此字段时反序列化为 Draft。
fn default_status() -> EntryStatus {
    EntryStatus::Draft
}

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
    /// 页面生命周期状态，新建默认 Draft；历史 JSON 无该字段时也回落 Draft。
    #[serde(default = "default_status")]
    pub status: EntryStatus,
    /// 页面**首次创建**时间戳。用于 `min_age_days` 晋升条件，不随正文编辑改变。
    /// 历史 JSON 无该字段时为 `None`，引擎会回落到 `updated_at`（保守旧行为）。
    #[serde(default)]
    pub created_at: Option<OffsetDateTime>,
    /// 进入**当前 `status`** 的时间戳。用于 `cooldown_days` 冷静期判定，仅在
    /// `status` 真正发生变化时更新；普通内容编辑不重置。历史 JSON 无该字段时为
    /// `None`，引擎会回落到 `updated_at`。
    #[serde(default)]
    pub status_entered_at: Option<OffsetDateTime>,
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
            status: EntryStatus::Draft,
            created_at: Some(now),
            status_entered_at: Some(now),
        }
    }

    /// Builder：显式绑定条目类型，让该页参与 lint 的完整度检查。
    pub fn with_entry_type(mut self, entry_type: EntryType) -> Self {
        self.entry_type = Some(entry_type);
        self
    }

    /// Builder：显式设置生命周期状态。
    ///
    /// 注意：会同步把 `status_entered_at` 重置为 **now**，保证新建页面的 cooldown 从
    /// 正确的时间起算。
    pub fn with_status(mut self, status: EntryStatus) -> Self {
        self.status = status;
        self.status_entered_at = Some(OffsetDateTime::now_utc());
        self
    }

    /// 读取 `min_age_days` 比较的起算时间；历史 JSON 回落到 `updated_at`。
    pub fn age_from(&self) -> OffsetDateTime {
        self.created_at.unwrap_or(self.updated_at)
    }

    /// 读取 `cooldown_days` 比较的起算时间；历史 JSON 回落到 `updated_at`。
    pub fn status_since(&self) -> OffsetDateTime {
        self.status_entered_at.unwrap_or(self.updated_at)
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
    use super::{extract_headings, extract_wikilinks, WikiPage};
    use crate::model::Scope;
    use crate::schema::{EntryStatus, EntryType};

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

    fn private_scope() -> Scope {
        Scope::Private {
            agent_id: "test".into(),
        }
    }

    #[test]
    fn page_status_default_is_draft() {
        // 新建 page 默认 Draft
        let page = WikiPage::new("Title", "body", private_scope());
        assert_eq!(page.status, EntryStatus::Draft);
    }

    #[test]
    fn page_status_with_status_works() {
        // with_status 链式调用正确写入
        let page =
            WikiPage::new("Title", "body", private_scope()).with_status(EntryStatus::Approved);
        assert_eq!(page.status, EntryStatus::Approved);
    }

    #[test]
    fn page_status_old_json_deserializes_to_draft() {
        // 旧 JSON 无 status 字段 → 反序列化得 Draft
        let json = r#"{
            "id": {"0": [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]},
            "title": "Old Page",
            "markdown": "body",
            "scope": {"kind": "private", "agent_id": "cli"},
            "updated_at": "2024-01-01T00:00:00Z",
            "outbound_page_titles": [],
            "entry_type": null
        }"#;
        // serde_json 能反序列化 PageId 的 UUID，使用标准 UUID 格式
        let json2 = r#"{
            "id": "00000000-0000-0000-0000-000000000000",
            "title": "Old Page",
            "markdown": "body",
            "scope": {"kind": "private", "agent_id": "cli"},
            "updated_at": "2024-01-01T00:00:00Z",
            "outbound_page_titles": [],
            "entry_type": null
        }"#;
        // 先序列化一个已有 page，然后去掉 status 字段再反序列化
        let page = WikiPage::new("Old Page", "body", private_scope());
        let mut v: serde_json::Value = serde_json::to_value(&page).unwrap();
        v.as_object_mut().unwrap().remove("status");
        let deserialized: WikiPage = serde_json::from_value(v).unwrap();
        assert_eq!(deserialized.status, EntryStatus::Draft);
        // 确认 json2 变量不被 unused 警告
        let _ = json;
        let _ = json2;
    }

    #[test]
    fn page_status_with_entry_type_chains() {
        // with_entry_type + with_status 链式组合
        let page = WikiPage::new("T", "md", private_scope())
            .with_entry_type(EntryType::Concept)
            .with_status(EntryStatus::InReview);
        assert_eq!(page.entry_type, Some(EntryType::Concept));
        assert_eq!(page.status, EntryStatus::InReview);
    }
}
