//! 跨库引用解析：把正文里的链接 resolve 成"内部边（页→页）" or "外部边（页→source）"。
//!
//! 算法：
//!
//! 1. **UUID 主索引**：所有页（Wiki + X + 微信）按 `notion_uuid` 建索引。
//!    Internal link 的 `target_uuid` 直接查表 → 内部边。
//! 2. **URL 副索引**：Source 页（X + 微信）按"文章链接"属性归一化后的 URL 建索引。
//!    Wiki 页里的外部 http(s) 链接先归一化再查表：
//!    - 命中 → 外部边（Wiki 页 → Source 页）
//!    - 未命中 → 记为 `unresolved_external`
//! 3. **Wiki.源文章URL 字段兜底**：当 properties 里有 `源文章URL: https://www.notion.so/{uuid}` 时，
//!    抽 UUID 直接查 Source 索引（notion_uuid 命中）→ 外部边。

use crate::model::{LibraryKind, LinkKind, RawPage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 归一化的边（resolve 阶段输出）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from_uuid: String,
    pub to_uuid: String,
    pub kind: EdgeKind,
    /// 命中依据，便于审计
    pub matched_by: MatchReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeKind {
    /// Wiki 内部引用（summary→concept 等）
    Internal,
    /// Wiki → Source（指向 X/微信 原文）
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchReason {
    /// 通过相对路径文件名里的 UUID 直接命中
    PathUuid,
    /// 通过 Wiki.源文章URL 字段的 notion.so UUID 命中
    SourceUrlField,
    /// 通过归一化 URL 命中 Source.文章链接
    NormalizedUrl,
}

/// 未解析的外链（可能是真正的外部参考，也可能是坏数据）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedExternal {
    pub from_uuid: String,
    pub href: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResolveStats {
    pub total_pages: usize,
    pub total_links: usize,
    pub internal_resolved: usize,
    pub internal_unresolved: usize,
    pub external_resolved_by_url: usize,
    pub external_resolved_by_source_url_field: usize,
    pub external_unresolved: usize,
}

pub struct Resolved {
    pub edges: Vec<Edge>,
    pub unresolved: Vec<UnresolvedExternal>,
    pub stats: ResolveStats,
}

/// 入口：对所有页跑 resolve。
pub fn resolve(pages: &[RawPage]) -> Resolved {
    // --- 1. 建 UUID 索引 ---
    let mut uuid_index: HashMap<&str, &RawPage> = HashMap::new();
    for p in pages {
        uuid_index.insert(p.notion_uuid.as_str(), p);
    }

    // --- 2. 建 URL→UUID 索引（只收 source 库的 `文章链接`） ---
    let mut url_to_uuid: HashMap<String, String> = HashMap::new();
    for p in pages {
        if !matches!(p.library, LibraryKind::XBookmark | LibraryKind::WeChat) {
            continue;
        }
        if let Some(article_url) = get_property(p, "文章链接") {
            let norm = normalize_url(article_url);
            if !norm.is_empty() {
                // 说明：如果多条 source 撞同一 URL，后者会覆盖前者。
                // 这种情况由后续去重阶段处理，这里不报警（去重在真正写盘时做）。
                url_to_uuid.insert(norm, p.notion_uuid.clone());
            }
        }
    }

    // --- 3. 逐页 resolve ---
    let mut edges: Vec<Edge> = Vec::new();
    let mut unresolved: Vec<UnresolvedExternal> = Vec::new();
    let mut stats = ResolveStats {
        total_pages: pages.len(),
        ..Default::default()
    };

    for p in pages {
        // 3a. Wiki.源文章URL 字段兜底
        if p.library == LibraryKind::Wiki {
            if let Some(src_url) = get_property(p, "源文章URL") {
                if let Some(uuid) = extract_notion_uuid_from_url(src_url) {
                    if uuid_index.contains_key(uuid.as_str()) {
                        edges.push(Edge {
                            from_uuid: p.notion_uuid.clone(),
                            to_uuid: uuid,
                            kind: EdgeKind::External,
                            matched_by: MatchReason::SourceUrlField,
                        });
                        stats.external_resolved_by_source_url_field += 1;
                    }
                }
            }
        }

        // 3b. 正文链接
        for link in &p.links {
            stats.total_links += 1;
            match link.kind {
                LinkKind::Internal => {
                    if let Some(uuid) = &link.target_uuid {
                        if uuid_index.contains_key(uuid.as_str()) {
                            edges.push(Edge {
                                from_uuid: p.notion_uuid.clone(),
                                to_uuid: uuid.clone(),
                                kind: EdgeKind::Internal,
                                matched_by: MatchReason::PathUuid,
                            });
                            stats.internal_resolved += 1;
                        } else {
                            stats.internal_unresolved += 1;
                        }
                    } else {
                        stats.internal_unresolved += 1;
                    }
                }
                LinkKind::External => {
                    let norm = normalize_url(&link.href);
                    if norm.is_empty() {
                        continue;
                    }
                    if let Some(uuid) = url_to_uuid.get(&norm) {
                        edges.push(Edge {
                            from_uuid: p.notion_uuid.clone(),
                            to_uuid: uuid.clone(),
                            kind: EdgeKind::External,
                            matched_by: MatchReason::NormalizedUrl,
                        });
                        stats.external_resolved_by_url += 1;
                    } else {
                        unresolved.push(UnresolvedExternal {
                            from_uuid: p.notion_uuid.clone(),
                            href: link.href.clone(),
                            text: link.text.clone(),
                        });
                        stats.external_unresolved += 1;
                    }
                }
            }
        }
    }

    Resolved {
        edges,
        unresolved,
        stats,
    }
}

pub fn get_property<'a>(page: &'a RawPage, key: &str) -> Option<&'a str> {
    page.properties
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// 从 Notion 页 URL 里抽 UUID。
///
/// 形如：
/// - `https://www.notion.so/33f701074b68814cad72d32f2c02a093`
/// - `https://www.notion.so/某页标题-33f701074b68814cad72d32f2c02a093`
pub fn extract_notion_uuid_from_url(url: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?i)([0-9a-f]{32})").ok()?;
    re.captures(url).map(|c| c[1].to_lowercase())
}

/// URL 归一化：统一 scheme/host 大小写、去锚点、去常见跟踪参数。
///
/// 说明：本工具只服务"是否是同一篇文章"的判断，不追求完全 RFC 3986 归一化。
pub fn normalize_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    // 去掉 `#anchor`
    let (before_anchor, _) = match trimmed.find('#') {
        Some(i) => trimmed.split_at(i),
        None => (trimmed, ""),
    };

    // scheme+host 统一小写，path+query 保留原样（微信 __biz= 参数大小写敏感）
    if let Some(scheme_end) = before_anchor.find("://") {
        let scheme = &before_anchor[..scheme_end];
        let rest = &before_anchor[scheme_end + 3..];
        let (host_and_path, _) = match rest.find('/') {
            Some(i) => rest.split_at(i),
            None => (rest, ""),
        };
        let host = host_and_path.to_ascii_lowercase();
        let tail = &rest[host_and_path.len()..];
        let mut out = String::new();
        out.push_str(&scheme.to_ascii_lowercase());
        out.push_str("://");
        out.push_str(&host);
        out.push_str(tail);
        // 去掉尾部斜杠（`https://x.com/foo/` → `https://x.com/foo`）
        if out.ends_with('/') && out.matches('/').count() > 3 {
            out.pop();
        }
        out
    } else {
        before_anchor.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 归一化_url_去锚点_小写_host() {
        assert_eq!(
            normalize_url("https://X.com/Foo/status/123#anchor"),
            "https://x.com/Foo/status/123"
        );
    }

    #[test]
    fn 归一化_url_保留微信__biz_大小写() {
        let input = "https://mp.weixin.qq.com/s?__biz=MzU&mid=1";
        assert_eq!(normalize_url(input), input);
    }

    #[test]
    fn 从_notion_url_抽_uuid() {
        assert_eq!(
            extract_notion_uuid_from_url("https://www.notion.so/33f701074b68814cad72d32f2c02a093")
                .as_deref(),
            Some("33f701074b68814cad72d32f2c02a093")
        );
    }
}
