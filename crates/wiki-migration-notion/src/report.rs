//! 干跑报告：把解析和 resolve 的结果渲染成人类可读的 markdown。

use crate::model::{LibraryKind, RawPage};
use crate::resolver::{get_property, Resolved};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use wiki_core::schema::{EntryStatus, EntryType};

pub fn render_report(pages: &[RawPage], resolved: &Resolved) -> String {
    let mut out = String::new();

    // --- 标题 + 总览 ---
    let _ = writeln!(out, "# Notion 迁移干跑报告\n");
    let _ = writeln!(
        out,
        "- 总页数：**{}**（Wiki / X / 微信 见下）",
        resolved.stats.total_pages
    );
    let _ = writeln!(out, "- 正文链接总数：**{}**", resolved.stats.total_links);
    let _ = writeln!(
        out,
        "- 内部边已解析 / 未解析：**{} / {}**",
        resolved.stats.internal_resolved, resolved.stats.internal_unresolved
    );
    let _ = writeln!(
        out,
        "- 外部边（按正文 URL 命中 source）：**{}**",
        resolved.stats.external_resolved_by_url
    );
    let _ = writeln!(
        out,
        "- 外部边（按 Wiki.源文章URL 字段命中）：**{}**",
        resolved.stats.external_resolved_by_source_url_field
    );
    let _ = writeln!(
        out,
        "- 未解析外链：**{}**（见附录）\n",
        resolved.stats.external_unresolved
    );

    // --- 按库统计 ---
    let mut lib_count: BTreeMap<&str, usize> = BTreeMap::new();
    for p in pages {
        *lib_count.entry(p.library.as_str()).or_default() += 1;
    }
    let _ = writeln!(out, "## 按库分布\n");
    let _ = writeln!(out, "| 库 | 条目数 |");
    let _ = writeln!(out, "| --- | ---: |");
    for (lib, n) in &lib_count {
        let _ = writeln!(out, "| {} | {} |", lib, n);
    }
    let _ = writeln!(out);

    // --- Wiki 类型×状态矩阵 ---
    let _ = writeln!(out, "## Wiki 类型 × 状态矩阵\n");
    let mut matrix: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut unknown_type: Vec<&RawPage> = Vec::new();
    let mut unknown_status: Vec<&RawPage> = Vec::new();

    for p in pages.iter().filter(|p| p.library == LibraryKind::Wiki) {
        let raw_type = get_property(p, "类型").unwrap_or("<missing>");
        let raw_status = get_property(p, "状态").unwrap_or("<missing>");
        let type_key = match EntryType::parse(raw_type) {
            Ok(t) => format!("{:?}", t),
            Err(_) => {
                unknown_type.push(p);
                format!("<未知:{raw_type}>")
            }
        };
        let status_key = match EntryStatus::parse(raw_status) {
            Ok(s) => format!("{:?}", s),
            Err(_) => {
                unknown_status.push(p);
                format!("<未知:{raw_status}>")
            }
        };
        *matrix
            .entry(type_key)
            .or_default()
            .entry(status_key)
            .or_default() += 1;
    }

    // 表头：收集所有出现过的 status
    let mut all_statuses: BTreeMap<String, ()> = BTreeMap::new();
    for row in matrix.values() {
        for k in row.keys() {
            all_statuses.insert(k.clone(), ());
        }
    }
    let status_cols: Vec<&String> = all_statuses.keys().collect();

    let _ = write!(out, "| 类型 |");
    for s in &status_cols {
        let _ = write!(out, " {} |", s);
    }
    let _ = writeln!(out, " 合计 |");
    let _ = write!(out, "| --- |");
    for _ in &status_cols {
        let _ = write!(out, " ---: |");
    }
    let _ = writeln!(out, " ---: |");

    for (ty, row) in &matrix {
        let _ = write!(out, "| {} |", ty);
        let mut total = 0;
        for s in &status_cols {
            let n = row.get(*s).copied().unwrap_or(0);
            total += n;
            let _ = write!(out, " {} |", n);
        }
        let _ = writeln!(out, " **{}** |", total);
    }
    let _ = writeln!(out);

    if !unknown_type.is_empty() {
        let _ = writeln!(
            out,
            "⚠️ **未识别的类型**：{} 条（抽样 5 条）\n",
            unknown_type.len()
        );
        for p in unknown_type.iter().take(5) {
            let _ = writeln!(
                out,
                "- `{}` → `类型: {:?}`",
                p.title,
                get_property(p, "类型")
            );
        }
        let _ = writeln!(out);
    }

    // --- Source 统计 ---
    let _ = writeln!(out, "## Source 库状态\n");
    for lib in [LibraryKind::XBookmark, LibraryKind::WeChat] {
        let total = pages.iter().filter(|p| p.library == lib).count();
        let with_url = pages
            .iter()
            .filter(|p| p.library == lib && get_property(p, "文章链接").is_some())
            .count();
        let compiled = pages
            .iter()
            .filter(|p| p.library == lib && get_property(p, "已编译到Wiki") == Some("Yes"))
            .count();
        let _ = writeln!(
            out,
            "- **{}**: 总 {} / 有链接 {} / 已编译到Wiki=Yes {}",
            lib.as_str(),
            total,
            with_url,
            compiled
        );
    }
    let _ = writeln!(out);

    // --- 未解析外链（按域名分组） ---
    let _ = writeln!(out, "## 未解析外链（按域名 top 15）\n");
    let mut by_host: BTreeMap<String, usize> = BTreeMap::new();
    for u in &resolved.unresolved {
        let host = extract_host(&u.href);
        *by_host.entry(host).or_default() += 1;
    }
    let mut v: Vec<_> = by_host.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    let _ = writeln!(out, "| 域名 | 次数 |");
    let _ = writeln!(out, "| --- | ---: |");
    for (h, n) in v.iter().take(15) {
        let _ = writeln!(out, "| `{}` | {} |", h, n);
    }
    let _ = writeln!(out);

    // --- 孤儿 source（没被任何 Wiki 页引用） ---
    use std::collections::HashSet;
    let referenced: HashSet<&str> = resolved
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::resolver::EdgeKind::External))
        .map(|e| e.to_uuid.as_str())
        .collect();

    let orphan_source_count = pages
        .iter()
        .filter(|p| {
            matches!(p.library, LibraryKind::XBookmark | LibraryKind::WeChat)
                && !referenced.contains(p.notion_uuid.as_str())
        })
        .count();

    let compiled_but_orphan = pages
        .iter()
        .filter(|p| {
            matches!(p.library, LibraryKind::XBookmark | LibraryKind::WeChat)
                && get_property(p, "已编译到Wiki") == Some("Yes")
                && !referenced.contains(p.notion_uuid.as_str())
        })
        .count();

    let _ = writeln!(out, "## 孤儿 Source\n");
    let _ = writeln!(
        out,
        "- 总孤儿 source（无任何 Wiki 页引用）：**{}**",
        orphan_source_count
    );
    let _ = writeln!(
        out,
        "- 其中标注 `已编译到Wiki=Yes` 但找不到引用的：**{}**（最值得审计）",
        compiled_but_orphan
    );
    let _ = writeln!(out);

    out
}

fn extract_host(url: &str) -> String {
    if let Some(after_scheme) = url.split_once("://") {
        let rest = after_scheme.1;
        let end = rest
            .find(|c: char| c == '/' || c == '?' || c == '#')
            .unwrap_or(rest.len());
        return rest[..end].to_ascii_lowercase();
    }
    "<no-scheme>".into()
}
