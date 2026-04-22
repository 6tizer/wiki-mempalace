//! Phase 4：把解析后的页面写入本地 Wiki 目录。
//!
//! 输出结构：
//!
//! ```text
//! <out>/
//!   pages/
//!     concept/   entity/   summary/   synthesis/   qa/   lint-report/   index/
//!   sources/
//!     x/   wechat/
//!   .wiki/
//!     uuid-map.json      ← Notion UUID → 相对文件路径
//!     migration-stats.json
//! ```
//!
//! 每个 .md 带 YAML frontmatter + 正文（正文里的 Notion 链接改写成 `[[wikilink]]`
//! 或 Markdown 相对路径）。

use crate::model::{LibraryKind, RawPage};
use crate::resolver::{get_property, normalize_url};
use anyhow::{bail, Result};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

/// 迁移选项
pub struct WriteOptions {
    /// 输出根目录
    pub out_dir: PathBuf,
    /// 干净外链的最小阈值：host 形如 `xxx.md` 被视为 Notion 自动链接化的 bug，过滤为纯文本
    pub filter_dotmd_pseudo_urls: bool,
    /// 微信图片 CDN / 其它资源 CDN：保留原样（图片语法），不当链接记录
    pub keep_image_hosts: bool,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            out_dir: PathBuf::from("/tmp/wiki-migrated"),
            filter_dotmd_pseudo_urls: true,
            keep_image_hosts: true,
        }
    }
}

/// 最终单条页面在输出端的相对路径（相对 out_dir）
#[derive(Debug, Clone, Serialize)]
pub struct PageLocation {
    pub notion_uuid: String,
    pub relative_path: String,
    pub title: String,
    pub kind: PageLocationKind,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum PageLocationKind {
    /// pages/{entry_type}/
    WikiPage,
    /// sources/{origin}/
    Source,
}

#[derive(Debug, Default, Serialize)]
pub struct WriteStats {
    pub wiki_pages_written: usize,
    pub sources_written: usize,
    pub orphan_sources: usize,
    pub mentions_rewritten: usize,
    pub external_rewritten_to_source: usize,
    pub pseudo_urls_cleaned: usize,
    pub filename_collisions_resolved: usize,
}

/// 入口
pub fn write_all(pages: &[RawPage], opts: &WriteOptions) -> Result<WriteStats> {
    if !opts.out_dir.exists() {
        std::fs::create_dir_all(&opts.out_dir)?;
    } else if opts.out_dir.read_dir()?.next().is_some() {
        bail!(
            "输出目录非空：{}。请先清空或指定新目录。",
            opts.out_dir.display()
        );
    }

    // --- 1. 预分配位置（决定每条记录落到哪个相对路径） ---
    let locations = allocate_locations(pages);

    // --- 2. 预计算：被引用过的 source UUID 集合（用于 orphan 标记） ---
    let referenced_sources = compute_referenced_sources(pages, &locations);

    // --- 3. 建 URL → location 索引（用于外部链接改写） ---
    let mut url_index: HashMap<String, &PageLocation> = HashMap::new();
    for p in pages {
        if !matches!(p.library, LibraryKind::XBookmark | LibraryKind::WeChat) {
            continue;
        }
        if let Some(url) = get_property(p, "文章链接") {
            let norm = normalize_url(url);
            if !norm.is_empty() {
                if let Some(loc) = locations.get(&p.notion_uuid) {
                    url_index.insert(norm, loc);
                }
            }
        }
    }

    // --- 4. 逐条写文件 ---
    let mut stats = WriteStats::default();
    for p in pages {
        let Some(loc) = locations.get(&p.notion_uuid) else {
            continue;
        };
        let content = render_page(
            p,
            loc,
            &locations,
            &url_index,
            &referenced_sources,
            opts,
            &mut stats,
        );
        let full = opts.out_dir.join(&loc.relative_path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full, content)?;
        match loc.kind {
            PageLocationKind::WikiPage => stats.wiki_pages_written += 1,
            PageLocationKind::Source => {
                stats.sources_written += 1;
                if !referenced_sources.contains(p.notion_uuid.as_str()) {
                    stats.orphan_sources += 1;
                }
            }
        }
    }

    // --- 5. 元数据 ---
    let meta_dir = opts.out_dir.join(".wiki");
    std::fs::create_dir_all(&meta_dir)?;
    let uuid_map: BTreeMap<&str, &str> = locations
        .iter()
        .map(|(u, l)| (u.as_str(), l.relative_path.as_str()))
        .collect();
    std::fs::write(
        meta_dir.join("uuid-map.json"),
        serde_json::to_string_pretty(&uuid_map)?,
    )?;
    std::fs::write(
        meta_dir.join("migration-stats.json"),
        serde_json::to_string_pretty(&stats)?,
    )?;

    Ok(stats)
}

/// 分配每页的落地位置。确保所有文件名在其 bucket 内 unique。
fn allocate_locations(pages: &[RawPage]) -> HashMap<String, PageLocation> {
    let mut by_bucket: BTreeMap<String, Vec<&RawPage>> = BTreeMap::new();
    for p in pages {
        let bucket = bucket_of(p);
        by_bucket.entry(bucket).or_default().push(p);
    }

    let mut out: HashMap<String, PageLocation> = HashMap::new();
    for (bucket, group) in by_bucket {
        let kind = if bucket.starts_with("pages/") {
            PageLocationKind::WikiPage
        } else {
            PageLocationKind::Source
        };
        let mut used: HashSet<String> = HashSet::new();
        for p in group {
            let mut base = slugify(&p.title);
            if base.is_empty() {
                base = "untitled".into();
            }
            // 第一轮：base.md；冲突则加 uuid8；再冲突则用完整 uuid 兜底
            let short = &p.notion_uuid[..8.min(p.notion_uuid.len())];
            let candidates = [
                format!("{}.md", base),
                format!("{}-{}.md", base, short),
                format!("{}-{}.md", base, &p.notion_uuid),
            ];
            let mut chosen: Option<String> = None;
            for c in &candidates {
                if !used.contains(c) {
                    chosen = Some(c.clone());
                    break;
                }
            }
            let name = chosen.unwrap_or_else(|| format!("{}.md", p.notion_uuid));
            used.insert(name.clone());
            let relative_path = format!("{}/{}", bucket, name);
            out.insert(
                p.notion_uuid.clone(),
                PageLocation {
                    notion_uuid: p.notion_uuid.clone(),
                    relative_path,
                    title: p.title.clone(),
                    kind,
                },
            );
        }
    }
    out
}

fn bucket_of(p: &RawPage) -> String {
    match p.library {
        LibraryKind::XBookmark => "sources/x".into(),
        LibraryKind::WeChat => "sources/wechat".into(),
        LibraryKind::Wiki => {
            let raw_type = get_property(p, "类型").unwrap_or("");
            let slug = match wiki_core::schema::EntryType::parse(raw_type) {
                Ok(wiki_core::schema::EntryType::Concept) => "concept",
                Ok(wiki_core::schema::EntryType::Entity) => "entity",
                Ok(wiki_core::schema::EntryType::Summary) => "summary",
                Ok(wiki_core::schema::EntryType::Synthesis) => "synthesis",
                Ok(wiki_core::schema::EntryType::Qa) => "qa",
                Ok(wiki_core::schema::EntryType::LintReport) => "lint-report",
                Ok(wiki_core::schema::EntryType::Index) => "index",
                // 未识别类型（`None` 等）统一归 index（用户确认的默认方案）
                Err(_) => "index",
            };
            format!("pages/{}", slug)
        }
    }
}

/// 将标题转成安全的 slug（保留中文字符，去 Windows/macOS 禁用字符和空白归一化）
fn slugify(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut last_sep = true;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() || (!c.is_ascii() && !c.is_control()) {
            out.push(c);
            last_sep = false;
        } else if matches!(c, '-' | '_') {
            out.push(c);
            last_sep = false;
        } else {
            // 其它字符（空白/标点/slash 等）折叠为单个 `-`
            if !last_sep {
                out.push('-');
                last_sep = true;
            }
        }
    }
    // 截去首尾的 `-`
    let trimmed = out.trim_matches('-').to_string();
    // 文件名不宜太长：按字符数限到 80
    let mut result = String::new();
    for (i, c) in trimmed.chars().enumerate() {
        if i >= 80 {
            break;
        }
        result.push(c);
    }
    result
}

/// 被引用过的 source uuid 集合
fn compute_referenced_sources(
    pages: &[RawPage],
    locations: &HashMap<String, PageLocation>,
) -> HashSet<String> {
    let mut set = HashSet::new();
    let url_index: HashMap<String, String> = pages
        .iter()
        .filter(|p| matches!(p.library, LibraryKind::XBookmark | LibraryKind::WeChat))
        .filter_map(|p| {
            get_property(p, "文章链接").map(|url| {
                let norm = normalize_url(url);
                (norm, p.notion_uuid.clone())
            })
        })
        .filter(|(u, _)| !u.is_empty())
        .collect();

    for p in pages.iter().filter(|p| p.library == LibraryKind::Wiki) {
        // 源文章URL 字段
        if let Some(src_url) = get_property(p, "源文章URL") {
            if let Some(uuid) = crate::resolver::extract_notion_uuid_from_url(src_url) {
                if locations.contains_key(&uuid) {
                    set.insert(uuid);
                }
            }
        }
        // 正文链接
        for l in &p.links {
            if matches!(l.kind, crate::model::LinkKind::Internal) {
                if let Some(u) = &l.target_uuid {
                    if locations
                        .get(u)
                        .map(|loc| loc.kind == PageLocationKind::Source)
                        .unwrap_or(false)
                    {
                        set.insert(u.clone());
                    }
                }
            } else if let Some(target) = url_index.get(&normalize_url(&l.href)) {
                set.insert(target.clone());
            }
        }
    }
    set
}

fn render_page(
    p: &RawPage,
    loc: &PageLocation,
    locations: &HashMap<String, PageLocation>,
    url_index: &HashMap<String, &PageLocation>,
    referenced_sources: &HashSet<String>,
    opts: &WriteOptions,
    stats: &mut WriteStats,
) -> String {
    let mut out = String::new();

    // --- frontmatter ---
    out.push_str("---\n");
    fm(&mut out, "title", &p.title);
    fm(&mut out, "notion_uuid", &p.notion_uuid);

    match p.library {
        LibraryKind::Wiki => {
            let entry_type = get_property(p, "类型")
                .and_then(|v| wiki_core::schema::EntryType::parse(v).ok())
                .map(|t| format!("{t:?}").to_ascii_lowercase())
                .unwrap_or_else(|| "index".into());
            // LintReport 的 Debug 是 "LintReport"，统一成 snake_case
            let entry_type = entry_type.replace("lintreport", "lint_report");
            fm(&mut out, "entry_type", &entry_type);

            if let Some(status) = get_property(p, "状态").and_then(|v| {
                wiki_core::schema::EntryStatus::parse(v)
                    .ok()
                    .map(|s| format!("{s:?}").to_ascii_lowercase())
            }) {
                let status = status
                    .replace("inreview", "in_review")
                    .replace("needsupdate", "needs_update");
                fm(&mut out, "status", &status);
            }
            for key in ["置信度", "源文章URL", "来源标签"] {
                if let Some(v) = get_property(p, key) {
                    if !v.is_empty() {
                        fm(&mut out, key_to_en(key), v);
                    }
                }
            }
        }
        LibraryKind::XBookmark | LibraryKind::WeChat => {
            fm(&mut out, "kind", "source");
            fm(
                &mut out,
                "origin",
                if matches!(p.library, LibraryKind::XBookmark) {
                    "x"
                } else {
                    "wechat"
                },
            );
            for key in ["作者", "文章链接", "来源", "发布时间", "备注"] {
                if let Some(v) = get_property(p, key) {
                    if !v.is_empty() {
                        fm(&mut out, key_to_en(key), v);
                    }
                }
            }
            if let Some(v) = get_property(p, "已编译到Wiki") {
                fm_raw(
                    &mut out,
                    "compiled_to_wiki",
                    if v == "Yes" { "true" } else { "false" },
                );
            }
            let is_orphan = !referenced_sources.contains(&p.notion_uuid);
            fm_raw(&mut out, "orphan", if is_orphan { "true" } else { "false" });
        }
    }

    // 标签：从 `标签` 字段解析（逗号分隔）
    if let Some(tags) = get_property(p, "标签") {
        let parts: Vec<&str> = tags
            .split(|c| c == ',' || c == '，')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if !parts.is_empty() {
            out.push_str("tags: [");
            for (i, t) in parts.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&yaml_inline_str(t));
            }
            out.push_str("]\n");
        }
    }

    for key in ["创建时间", "最后编辑时间", "最后编译时间"] {
        if let Some(v) = get_property(p, key) {
            if !v.is_empty() {
                fm(&mut out, key_to_en(key), v);
            }
        }
    }

    out.push_str("---\n\n");

    // --- 正文标题（重复 H1 便于 Obsidian 显示） ---
    out.push_str("# ");
    out.push_str(&p.title);
    out.push_str("\n\n");

    // --- 正文内容（改写链接） ---
    let rewritten = rewrite_body(&p.body, loc, locations, url_index, opts, stats);
    out.push_str(&rewritten);
    if !rewritten.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn key_to_en(key: &str) -> &'static str {
    match key {
        "置信度" => "confidence",
        "源文章URL" => "source_url",
        "来源标签" => "source_tags",
        "作者" => "author",
        "文章链接" => "url",
        "来源" => "origin_label",
        "发布时间" => "published_at",
        "备注" => "notes",
        "创建时间" => "created_at",
        "最后编辑时间" => "updated_at",
        "最后编译时间" => "last_compiled_at",
        _ => "extra",
    }
}

fn fm(out: &mut String, key: &str, val: &str) {
    out.push_str(key);
    out.push_str(": ");
    out.push_str(&yaml_inline_str(val));
    out.push('\n');
}

/// Raw 版本：跳过转义（用于我们明确知道是 YAML bool/number 的场景）
fn fm_raw(out: &mut String, key: &str, val: &str) {
    out.push_str(key);
    out.push_str(": ");
    out.push_str(val);
    out.push('\n');
}

/// 轻量 YAML 行内字符串：如果含特殊字符就加双引号并转义。
fn yaml_inline_str(s: &str) -> String {
    let needs_quote = s.is_empty()
        || s.contains(|c: char| {
            matches!(
                c,
                ':' | '#' | '\n' | '"' | '\'' | '{' | '}' | '[' | ']' | '|' | '>'
            )
        })
        || s.starts_with(char::is_whitespace)
        || s.ends_with(char::is_whitespace)
        || matches!(s.trim(), "true" | "false" | "null" | "yes" | "no");
    if needs_quote {
        let escaped = s
            .replace('\\', r"\\")
            .replace('"', r#"\""#)
            .replace('\n', r"\n");
        format!("\"{escaped}\"")
    } else {
        s.to_string()
    }
}

/// 改写正文里的链接
fn rewrite_body(
    body: &str,
    from_loc: &PageLocation,
    locations: &HashMap<String, PageLocation>,
    url_index: &HashMap<String, &PageLocation>,
    opts: &WriteOptions,
    stats: &mut WriteStats,
) -> String {
    // 需要同时识别图片（`![...](...)` 保留原样）和普通链接
    let re_img = regex::Regex::new(r"!\[([^\]]*)\]\(([^)]+)\)").unwrap();
    let re_link = regex::Regex::new(r"\[([^\]]*)\]\(([^)]+)\)").unwrap();

    // 策略：先把图片替换成占位符，再改写链接，再把占位符还原
    let mut img_store: Vec<String> = Vec::new();
    let with_img_placeholders = re_img.replace_all(body, |caps: &regex::Captures| {
        let full = caps.get(0).unwrap().as_str().to_string();
        let idx = img_store.len();
        img_store.push(full);
        format!("\x00IMG{}\x00", idx)
    });

    let rewritten = re_link.replace_all(&with_img_placeholders, |caps: &regex::Captures| {
        let text = caps.get(1).unwrap().as_str();
        let href = caps.get(2).unwrap().as_str().trim();
        let full = caps.get(0).unwrap().as_str().to_string();

        // 1) 伪 URL 清洗：host 以 `.md` 结尾的（claude.md / skill.md 等）
        if opts.filter_dotmd_pseudo_urls {
            if let Some(host) = host_of(href) {
                if host.ends_with(".md") {
                    stats.pseudo_urls_cleaned += 1;
                    return text.to_string(); // 保留锚文本，丢掉链接
                }
            }
        }

        // 2) 内部相对路径（Notion 导出）：若 target uuid 在 locations 里，改写成相对路径
        if !href.starts_with("http") && !href.starts_with('#') {
            if let Some(uuid) = extract_uuid_in_href(href) {
                if let Some(target) = locations.get(&uuid) {
                    stats.mentions_rewritten += 1;
                    let rel = relative_path(&from_loc.relative_path, &target.relative_path);
                    return format!("[{}]({})", text, rel);
                }
                // 目标不在（极少数 46 条未解析内部边），保留锚文本
                return text.to_string();
            }
        }

        // 3) 外部 URL → 若能命中 source，改写成相对路径
        if href.starts_with("http://") || href.starts_with("https://") {
            let norm = normalize_url(href);
            if let Some(target) = url_index.get(&norm) {
                stats.external_rewritten_to_source += 1;
                let rel = relative_path(&from_loc.relative_path, &target.relative_path);
                return format!("[{}]({})", text, rel);
            }
        }

        // 4) 其它：原样
        full
    });

    // 还原图片
    let mut final_out = rewritten.into_owned();
    for (i, img) in img_store.iter().enumerate() {
        final_out = final_out.replace(&format!("\x00IMG{}\x00", i), img);
    }
    final_out
}

fn host_of(url: &str) -> Option<String> {
    let after = url.split_once("://")?.1;
    let end = after
        .find(|c: char| c == '/' || c == '?' || c == '#')
        .unwrap_or(after.len());
    Some(after[..end].to_ascii_lowercase())
}

fn extract_uuid_in_href(href: &str) -> Option<String> {
    let decoded = percent_encoding::percent_decode_str(href).decode_utf8_lossy();
    let re = regex::Regex::new(r"(?i)([0-9a-f]{32})\.md$").ok()?;
    re.captures(&decoded).map(|c| c[1].to_lowercase())
}

/// 计算 from → to 的 POSIX 风格相对路径
fn relative_path(from: &str, to: &str) -> String {
    let from_dir: Vec<&str> = from.split('/').collect();
    let from_dir = &from_dir[..from_dir.len() - 1];
    let to_parts: Vec<&str> = to.split('/').collect();

    // 找公共前缀
    let common = from_dir
        .iter()
        .zip(to_parts.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let mut out = String::new();
    for _ in common..from_dir.len() {
        out.push_str("../");
    }
    for (i, part) in to_parts.iter().enumerate().skip(common) {
        if i > common {
            out.push('/');
        }
        out.push_str(part);
    }
    if out.is_empty() {
        out.push_str(to_parts.last().unwrap_or(&""));
    }
    // URL 编码空格等
    out.replace(' ', "%20")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_保留中文_去掉标点() {
        assert_eq!(slugify("Gemma 4 全系列"), "Gemma-4-全系列");
        // 全角冒号是有效 macOS 文件名字符，保留（Obsidian 可识别）
        assert_eq!(slugify("摘要：Gemma 4 测评"), "摘要：Gemma-4-测评");
    }

    #[test]
    fn relative_path_跨目录() {
        assert_eq!(
            relative_path("pages/summary/a.md", "sources/x/b.md"),
            "../../sources/x/b.md"
        );
    }

    #[test]
    fn relative_path_同目录() {
        assert_eq!(
            relative_path("pages/concept/a.md", "pages/concept/b.md"),
            "b.md"
        );
    }

    #[test]
    fn yaml_inline_需要引号() {
        assert_eq!(yaml_inline_str("yes"), r#""yes""#);
        assert_eq!(yaml_inline_str("hello: world"), r#""hello: world""#);
        assert_eq!(yaml_inline_str("plain"), "plain");
    }

    #[test]
    fn host_of_提取() {
        assert_eq!(host_of("https://x.com/foo?bar"), Some("x.com".into()));
        assert_eq!(host_of("http://claude.md"), Some("claude.md".into()));
    }
}
