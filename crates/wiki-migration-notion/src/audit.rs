//! 孤儿 Source 审计模块。
//!
//! 从已迁移的 vault 目录中：
//! 1. 提取所有 `orphan: true` 的 source 元数据
//! 2. 对每条孤儿的标题在 Wiki 页正文中做模糊匹配
//! 3. 按 A/B/C 分类并生成审计报告

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

/// 孤儿 source 的元数据（从 frontmatter 提取）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrphanMeta {
    pub title: String,
    pub notion_uuid: String,
    pub origin: String,
    pub compiled_to_wiki: bool,
    pub url: Option<String>,
    pub tags: Vec<String>,
    pub relative_path: String,
}

/// 标题匹配结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TitleMatch {
    pub orphan_title: String,
    pub orphan_path: String,
    /// 匹配到的 Wiki 页列表：(相对路径, 匹配片段上下文)
    pub wiki_pages: Vec<(String, String)>,
}

/// 审计分类
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditCategory {
    /// A: 标题匹配到 — Wiki 正文确实提到了，但用纯文本无链接
    TitleMatched,
    /// B: 未匹配 + compiled=true — Notion 标记已编译但实际没提到
    CompiledButNotFound,
    /// C: compiled=false — 从未编译，当孤儿正常
    NeverCompiled,
}

/// 单条孤儿审计结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrphanAuditEntry {
    pub meta: OrphanMeta,
    pub category: AuditCategory,
    pub title_matches: Option<TitleMatch>,
}

/// 审计报告汇总统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStats {
    pub total_orphans: usize,
    pub category_a: usize,
    pub category_b: usize,
    pub category_c: usize,
    pub by_origin: HashMap<String, HashMap<String, usize>>,
}

/// 从 vault 目录扫描所有孤儿 source
pub fn scan_orphan_sources(vault_dir: &Path) -> Result<Vec<OrphanMeta>> {
    let sources_dir = vault_dir.join("sources");
    if !sources_dir.exists() {
        anyhow::bail!("sources 目录不存在：{}", sources_dir.display());
    }

    let re_frontmatter = Regex::new(r"(?s)^---\s*\n(.*?)\n---")?;
    let mut orphans = Vec::new();

    for entry in WalkDir::new(&sources_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let content = std::fs::read_to_string(path)?;
        let relative = path.strip_prefix(vault_dir)?.to_string_lossy().to_string();

        // 解析 frontmatter
        let Some(caps) = re_frontmatter.captures(&content) else {
            continue;
        };
        let fm_text = caps.get(1).unwrap().as_str();
        let fm = parse_simple_yaml(fm_text);

        // 只关注 orphan: true
        if fm.get("orphan").map(|v| v.as_str()) != Some("true") {
            continue;
        }

        let title = fm.get("title").cloned().unwrap_or_default();
        let notion_uuid = fm.get("notion_uuid").cloned().unwrap_or_default();
        let origin = fm.get("origin").cloned().unwrap_or_default();
        let compiled = fm
            .get("compiled_to_wiki")
            .map(|v| v == "true")
            .unwrap_or(false);
        let url = fm.get("url").cloned();
        let tags = fm
            .get("tags")
            .map(|v| {
                // 解析 [tag1, tag2, ...] 格式
                let trimmed = v.trim().trim_start_matches('[').trim_end_matches(']');
                trimmed
                    .split(',')
                    .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        orphans.push(OrphanMeta {
            title,
            notion_uuid,
            origin,
            compiled_to_wiki: compiled,
            url,
            tags,
            relative_path: relative,
        });
    }

    // 按来源排序，便于报告阅读
    orphans.sort_by(|a, b| (&a.origin, &a.title).cmp(&(&b.origin, &b.title)));
    Ok(orphans)
}

/// 从 vault 目录提取所有 Wiki 页面的正文（不含 frontmatter）
pub fn scan_wiki_pages(vault_dir: &Path) -> Result<Vec<(String, String)>> {
    let pages_dir = vault_dir.join("pages");
    if !pages_dir.exists() {
        anyhow::bail!("pages 目录不存在：{}", pages_dir.display());
    }

    let re_frontmatter = Regex::new(r"(?s)^---\s*\n.*?\n---\s*\n")?;
    let mut pages = Vec::new();

    for entry in WalkDir::new(&pages_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let content = std::fs::read_to_string(path)?;
        let relative = path.strip_prefix(vault_dir)?.to_string_lossy().to_string();

        // 去掉 frontmatter，只保留正文
        let body = re_frontmatter.replace(&content, "").to_string();

        pages.push((relative, body));
    }

    Ok(pages)
}

/// 对孤儿列表做标题模糊匹配，返回审计结果
pub fn audit_orphans(
    orphans: &[OrphanMeta],
    wiki_pages: &[(String, String)],
) -> Vec<OrphanAuditEntry> {
    let mut results = Vec::with_capacity(orphans.len());

    for orphan in orphans {
        // 只对 compiled=true 的做标题匹配（compiled=false 直接归 C）
        let title_matches = if orphan.compiled_to_wiki {
            find_title_matches(&orphan.title, wiki_pages)
        } else {
            None
        };

        let has_match = title_matches
            .as_ref()
            .map(|m| !m.wiki_pages.is_empty())
            .unwrap_or(false);

        let category = if has_match {
            AuditCategory::TitleMatched
        } else if orphan.compiled_to_wiki {
            AuditCategory::CompiledButNotFound
        } else {
            AuditCategory::NeverCompiled
        };

        results.push(OrphanAuditEntry {
            meta: orphan.clone(),
            category,
            title_matches,
        });
    }

    results
}

/// 在 Wiki 页正文中搜索标题子串
fn find_title_matches(orphan_title: &str, wiki_pages: &[(String, String)]) -> Option<TitleMatch> {
    // 生成多个搜索模式：完整标题、去标点后的核心子串、前 N 字符
    let patterns = generate_search_patterns(orphan_title);

    let mut hits: Vec<(String, String)> = Vec::new();

    for (wiki_path, body) in wiki_pages {
        for pattern in &patterns {
            if pattern.is_empty() || pattern.len() < 4 {
                continue;
            }
            // 不区分大小写搜索
            if let Some(byte_idx) = body.to_lowercase().find(&pattern.to_lowercase()) {
                // 提取匹配位置的上下文（前后各 40 字符）
                // byte_idx 是字节偏移，需转换为字符偏移
                let char_idx = body[..byte_idx].chars().count();
                let pat_char_len = pattern.chars().count();
                let body_char_len = body.chars().count();
                let char_start = char_idx.saturating_sub(40);
                let char_end = (char_idx + pat_char_len + 40).min(body_char_len);
                let context_str: String = body
                    .chars()
                    .skip(char_start)
                    .take(char_end - char_start)
                    .collect();
                let context = if char_start > 0 { "..." } else { "" }.to_string()
                    + &context_str
                    + if char_end < body_char_len { "..." } else { "" };

                // 去重：同一 wiki 页只保留第一个匹配
                if !hits.iter().any(|(p, _)| p == wiki_path) {
                    hits.push((wiki_path.clone(), context));
                }
                break; // 找到第一个匹配模式即可
            }
        }
    }

    if hits.is_empty() {
        return None;
    }

    Some(TitleMatch {
        orphan_title: orphan_title.to_string(),
        orphan_path: String::new(), // 调用方填充
        wiki_pages: hits,
    })
}

/// 从标题生成搜索用的子串模式
fn generate_search_patterns(title: &str) -> Vec<String> {
    let mut patterns = Vec::new();

    // 1. 完整标题（去掉常见前后缀标点）
    let cleaned = title
        .trim()
        .trim_matches(|c: char| c == '《' || c == '》' || c == '"' || c == '\'')
        .to_string();
    if !cleaned.is_empty() {
        patterns.push(cleaned.clone());
    }

    // 2. 去掉所有标点/空白，取纯文字核心
    let pure_chars: String = title
        .chars()
        .filter(|c| c.is_alphanumeric() || (!c.is_ascii() && !c.is_control()))
        .collect();
    if !pure_chars.is_empty() && pure_chars != cleaned {
        patterns.push(pure_chars);
    }

    // 3. 取标题的前 15 个字符（短标题截取可能匹配部分标题引用）
    let prefix: String = title.chars().take(15).collect();
    if prefix.len() >= 6 && !patterns.contains(&prefix) {
        patterns.push(prefix);
    }

    patterns
}

/// 计算审计统计
pub fn compute_stats(entries: &[OrphanAuditEntry]) -> AuditStats {
    let mut stats = AuditStats {
        total_orphans: entries.len(),
        category_a: 0,
        category_b: 0,
        category_c: 0,
        by_origin: HashMap::new(),
    };

    for entry in entries {
        match entry.category {
            AuditCategory::TitleMatched => stats.category_a += 1,
            AuditCategory::CompiledButNotFound => stats.category_b += 1,
            AuditCategory::NeverCompiled => stats.category_c += 1,
        }

        let origin = &entry.meta.origin;
        let cat_key = format!("{:?}", entry.category);
        let origin_map = stats.by_origin.entry(origin.clone()).or_default();
        *origin_map.entry(cat_key).or_default() += 1;
    }

    stats
}

/// 渲染 Markdown 审计报告
pub fn render_report(entries: &[OrphanAuditEntry], stats: &AuditStats) -> String {
    let mut out = String::new();

    out.push_str("# 孤儿 Source 审计报告\n\n");
    out.push_str(&format!("> 生成时间：{}\n", chrono_now()));
    out.push_str(&format!("> 孤儿总数：{}\n\n", stats.total_orphans));

    // 汇总表
    out.push_str("## 汇总统计\n\n");
    out.push_str("| 分类 | 数量 | 说明 |\n");
    out.push_str("| --- | --- | --- |\n");
    out.push_str(&format!(
        "| A. 标题匹配到 | {} | Wiki 正文确实提到了，但用纯文本无链接 |\n",
        stats.category_a
    ));
    out.push_str(&format!(
        "| B. 未匹配 + compiled=true | {} | Notion 标记已编译但找不到引用 |\n",
        stats.category_b
    ));
    out.push_str(&format!(
        "| C. compiled=false | {} | 从未编译，当孤儿正常 |\n",
        stats.category_c
    ));
    out.push('\n');

    // 按来源分布
    out.push_str("### 按来源分布\n\n");
    out.push_str("| 来源 | A | B | C | 合计 |\n");
    out.push_str("| --- | --- | --- | --- | --- |\n");
    for origin in &["x", "wechat"] {
        let m = stats.by_origin.get(*origin).cloned().unwrap_or_default();
        let a = m.get("TitleMatched").copied().unwrap_or(0);
        let b = m.get("CompiledButNotFound").copied().unwrap_or(0);
        let c = m.get("NeverCompiled").copied().unwrap_or(0);
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            origin,
            a,
            b,
            c,
            a + b + c
        ));
    }
    out.push('\n');

    // A 类明细
    let a_entries: Vec<_> = entries
        .iter()
        .filter(|e| e.category == AuditCategory::TitleMatched)
        .collect();
    if !a_entries.is_empty() {
        out.push_str("## A 类：标题匹配到（建议补链接）\n\n");
        for (i, entry) in a_entries.iter().enumerate() {
            out.push_str(&format!("### {}. {}\n\n", i + 1, entry.meta.title));
            out.push_str(&format!("- 来源：{}\n", entry.meta.origin));
            out.push_str(&format!("- 路径：`{}`\n", entry.meta.relative_path));
            if let Some(url) = &entry.meta.url {
                out.push_str(&format!("- URL：`{}`\n", url));
            }
            if !entry.meta.tags.is_empty() {
                out.push_str(&format!("- 标签：{}\n", entry.meta.tags.join(", ")));
            }
            if let Some(matches) = &entry.title_matches {
                out.push_str("- 匹配到的 Wiki 页：\n");
                for (wiki_path, context) in &matches.wiki_pages {
                    out.push_str(&format!("  - `{}`\n", wiki_path));
                    out.push_str(&format!("    > {}\n", context.replace('\n', " ")));
                }
            }
            out.push('\n');
        }
    }

    // B 类列表
    let b_entries: Vec<_> = entries
        .iter()
        .filter(|e| e.category == AuditCategory::CompiledButNotFound)
        .collect();
    if !b_entries.is_empty() {
        out.push_str("## B 类：已编译但未匹配（需人工确认）\n\n");
        out.push_str("| # | 标题 | 来源 | 标签 |\n");
        out.push_str("| --- | --- | --- | --- |\n");
        for (i, entry) in b_entries.iter().enumerate() {
            let tags = entry.meta.tags.join(", ");
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                i + 1,
                entry.meta.title,
                entry.meta.origin,
                tags
            ));
        }
        out.push('\n');
    }

    // C 类列表
    let c_entries: Vec<_> = entries
        .iter()
        .filter(|e| e.category == AuditCategory::NeverCompiled)
        .collect();
    if !c_entries.is_empty() {
        out.push_str("## C 类：未编译（正常孤儿）\n\n");
        out.push_str("| # | 标题 | 来源 |\n");
        out.push_str("| --- | --- | --- |\n");
        for (i, entry) in c_entries.iter().enumerate() {
            out.push_str(&format!(
                "| {} | {} | {} |\n",
                i + 1,
                entry.meta.title,
                entry.meta.origin
            ));
        }
        out.push('\n');
    }

    // 处理建议
    out.push_str("## 处理建议\n\n");
    out.push_str(&format!(
        "- A 类（{} 条）：在匹配到的 Wiki 页对应位置插入 source 链接\n",
        stats.category_a
    ));
    out.push_str(&format!(
        "- B 类（{} 条）：人工确认是否 Notion 标记错误，或是否存在其它关联方式\n",
        stats.category_b
    ));
    out.push_str(&format!(
        "- C 类（{} 条）：作为待处理队列保留，未来编译时再建链接\n",
        stats.category_c
    ));

    out
}

/// 补链接的统计
#[derive(Debug, Default, Serialize)]
pub struct FixStats {
    /// 处理的 A 类孤儿数
    pub orphans_processed: usize,
    /// 被修改的 Wiki 页面数
    pub wiki_pages_modified: usize,
    /// 插入的 source 链接数
    pub links_inserted: usize,
    /// 跳过的匹配（已有链接）
    pub skipped_already_linked: usize,
}

/// 对 A 类和 B1 类孤儿自动补链接。
///
/// 策略：
/// - A 类：利用审计 JSON 中已有的 title_matches 信息定位
/// - B1 类（已编译未匹配但归一化后能匹配到）：用归一化标题在 Wiki 正文搜索
///
/// 在匹配行尾追加 `（[source](相对路径)）` 格式的 Markdown 链接。
/// 已有 `[摘要：...](...)` 或 `[...](source)` 格式链接的跳过。
pub fn fix_orphans(vault_dir: &Path, audit_json_path: &Path) -> Result<FixStats> {
    let json_content = std::fs::read_to_string(audit_json_path)?;
    let audit_data: serde_json::Value = serde_json::from_str(&json_content)?;
    let empty_entries = vec![];
    let entries = audit_data["entries"].as_array().unwrap_or(&empty_entries);

    let mut stats = FixStats::default();
    let mut patches_by_file: HashMap<String, Vec<(usize, String)>> = HashMap::new();

    let re_frontmatter = Regex::new(r"(?s)^---\s*\n.*?\n---\s*\n")?;

    // 预加载所有 concept/entity/synthesis/qa 页面的正文（B1 归一化匹配用）
    let wiki_body_cache = load_wiki_bodies(vault_dir, &re_frontmatter)?;

    for entry in entries {
        let category = entry["category"].as_str().unwrap_or("");
        let title = entry["meta"]["title"].as_str().unwrap_or("");
        let orphan_path = entry["meta"]["relative_path"].as_str().unwrap_or("");

        match category {
            "TitleMatched" => {
                // A 类：使用已有的 title_matches
                let title_matches = entry.get("title_matches");
                let empty_wp = vec![];
                let wiki_pages = title_matches
                    .and_then(|m| m.get("wiki_pages"))
                    .and_then(|v| v.as_array())
                    .unwrap_or(&empty_wp);

                stats.orphans_processed += 1;

                for wp in wiki_pages {
                    let wiki_path = wp[0].as_str().unwrap_or("");
                    if wiki_path.starts_with("pages/summary/")
                        || wiki_path.starts_with("pages/lint-report/")
                    {
                        continue;
                    }

                    let full_path = vault_dir.join(wiki_path);
                    let Ok(content) = std::fs::read_to_string(&full_path) else {
                        continue;
                    };

                    let body_start = re_frontmatter.find(&content).map(|m| m.end()).unwrap_or(0);
                    let body = &content[body_start..];

                    let search_patterns = generate_search_patterns(title);
                    let mut best_pos: Option<usize> = None;
                    let mut best_len: usize = 0;

                    for pattern in &search_patterns {
                        if pattern.len() < 4 {
                            continue;
                        }
                        if let Some(byte_idx) = body.to_lowercase().find(&pattern.to_lowercase()) {
                            let context_start = byte_idx.saturating_sub(20);
                            let context_end = (byte_idx + pattern.len() + 80).min(body.len());

                            let context = char_safe_slice(body, context_start, context_end);

                            if has_existing_link(&context, title) {
                                stats.skipped_already_linked += 1;
                                continue;
                            }

                            if pattern.len() > best_len {
                                best_pos = Some(byte_idx);
                                best_len = pattern.len();
                            }
                        }
                    }

                    let Some(pos) = best_pos else { continue };

                    let rel = relative_path(wiki_path, orphan_path);
                    let link_text = format!("（[source]({rel})）");
                    let line_end = find_line_end(body, pos + best_len);
                    patches_by_file
                        .entry(wiki_path.to_string())
                        .or_default()
                        .push((body_start + line_end, link_text));
                    stats.links_inserted += 1;
                }
            }
            "CompiledButNotFound" => {
                // B1 类：归一化匹配
                let norm_title = normalize_for_match(title);
                if norm_title.len() < 6 {
                    continue;
                }

                let short: String = norm_title.chars().take(15).collect();
                if short.len() < 6 {
                    continue;
                }

                for (wiki_path, norm_body, _body_start_offset) in &wiki_body_cache {
                    if let Some(_idx) = norm_body.find(&short) {
                        let full_path = vault_dir.join(wiki_path);
                        let Ok(content) = std::fs::read_to_string(&full_path) else {
                            continue;
                        };

                        let fm_body_start =
                            re_frontmatter.find(&content).map(|m| m.end()).unwrap_or(0);
                        let body = &content[fm_body_start..];

                        // 用原始标题在正文中找精确位置
                        let patterns = generate_search_patterns(title);
                        let mut found_pos: Option<usize> = None;
                        let mut found_len: usize = 0;

                        // 先尝试原始标题匹配
                        for p in &patterns {
                            if p.len() < 4 {
                                continue;
                            }
                            if let Some(byte_idx) = body.to_lowercase().find(&p.to_lowercase()) {
                                let ctx_start = byte_idx.saturating_sub(20);
                                let ctx_end = (byte_idx + p.len() + 80).min(body.len());
                                let ctx = char_safe_slice(body, ctx_start, ctx_end);
                                if has_existing_link(&ctx, title) {
                                    stats.skipped_already_linked += 1;
                                    continue;
                                }
                                if p.len() > found_len {
                                    found_pos = Some(byte_idx);
                                    found_len = p.len();
                                }
                            }
                        }

                        // 如果原始标题找不到，用归一化匹配定位
                        if found_pos.is_none() {
                            let norm_body_orig = normalize_for_match(body);
                            if let Some(norm_idx) = norm_body_orig.find(&short) {
                                // 归一化索引转回原文字节索引
                                let byte_idx =
                                    norm_to_byte_idx(body, &norm_body_orig, norm_idx + short.len());
                                let ctx_start = byte_idx.saturating_sub(40);
                                let ctx_end = (byte_idx + 80).min(body.len());
                                let ctx = char_safe_slice(body, ctx_start, ctx_end);
                                if !has_existing_link(&ctx, title) {
                                    found_pos = Some(byte_idx);
                                    found_len = 0; // 行尾插入，不需要精确长度
                                } else {
                                    stats.skipped_already_linked += 1;
                                }
                            }
                        }

                        if let Some(pos) = found_pos {
                            let rel = relative_path(wiki_path, orphan_path);
                            let link_text = format!("（[source]({rel})）");
                            let line_end = if found_len > 0 {
                                find_line_end(body, pos + found_len)
                            } else {
                                find_line_end(body, pos)
                            };
                            patches_by_file
                                .entry(wiki_path.to_string())
                                .or_default()
                                .push((fm_body_start + line_end, link_text));
                            stats.links_inserted += 1;
                            stats.orphans_processed += 1;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // 应用补丁
    for (wiki_path, mut patches) in patches_by_file {
        let full_path = vault_dir.join(&wiki_path);
        let Ok(mut content) = std::fs::read_to_string(&full_path) else {
            continue;
        };

        patches.sort_by_key(|item| std::cmp::Reverse(item.0));

        for (pos, insert) in patches {
            let pos = find_char_boundary(&content, pos);
            content.insert_str(pos, &insert);
        }

        std::fs::write(&full_path, content)?;
        stats.wiki_pages_modified += 1;
    }

    Ok(stats)
}

/// 检查上下文中是否已有指向 source 或 summary 的链接
fn has_existing_link(context: &str, title: &str) -> bool {
    // 已有 [摘要：...](...) 格式
    if context.contains("[摘要") && context.contains("](") {
        return true;
    }
    // 已有 [标题片段](...) 格式（指向 source 的链接）
    let short_title: String = title.chars().take(10).collect();
    if context.contains(&format!("[{short_title}")) && context.contains("](") {
        return true;
    }
    // 已有 (../sources/ 或 ../summary/) 路径
    if context.contains("../sources/") || context.contains("../summary/") {
        return true;
    }
    false
}

/// 找到最近的合法 UTF-8 字符边界（不大于 pos）
fn find_char_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    let mut p = pos;
    while p > 0 && !s.is_char_boundary(p) {
        p -= 1;
    }
    p
}

/// 安全的字符级切片
fn char_safe_slice(s: &str, start: usize, end: usize) -> String {
    let start = find_char_boundary(s, start);
    let end = find_char_boundary(s, end.min(s.len()));
    s[start..end].to_string()
}

/// 从 pos 开始找到最近的换行符位置（即当前行的末尾）
fn find_line_end(body: &str, pos: usize) -> usize {
    let pos = find_char_boundary(body, pos);
    if let Some(nl) = body[pos..].find('\n') {
        pos + nl
    } else {
        body.len()
    }
}

/// 计算从 wiki 页面到 source 文件的相对路径
fn relative_path(from: &str, to: &str) -> String {
    let from_dir: Vec<&str> = from.split('/').collect();
    let from_dir = &from_dir[..from_dir.len() - 1];
    let to_parts: Vec<&str> = to.split('/').collect();

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
    out.replace(' ', "%20")
}

/// 预加载所有 concept/entity/synthesis/qa 页面的归一化正文
fn load_wiki_bodies(
    vault_dir: &Path,
    re_frontmatter: &Regex,
) -> Result<Vec<(String, String, usize)>> {
    let pages_dir = vault_dir.join("pages");
    let mut result = Vec::new();

    if !pages_dir.exists() {
        return Ok(result);
    }

    for entry in walkdir::WalkDir::new(&pages_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let rel = path.strip_prefix(vault_dir)?.to_string_lossy().to_string();
        // 只加载 concept/entity/synthesis/qa
        if rel.starts_with("pages/summary/") || rel.starts_with("pages/lint-report/") {
            continue;
        }
        let content = std::fs::read_to_string(path)?;
        let body_start = re_frontmatter.find(&content).map(|m| m.end()).unwrap_or(0);
        let body = &content[body_start..];
        let norm = normalize_for_match(body);
        result.push((rel, norm, body_start));
    }

    Ok(result)
}

/// 归一化：去 emoji、去标点、只保留字母数字和中文，小写
fn normalize_for_match(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(&c) {
            out.extend(c.to_lowercase());
        }
    }
    out
}

/// 将归一化字符串的索引转换回原字符串的字节偏移
fn norm_to_byte_idx(orig: &str, _norm: &str, norm_idx: usize) -> usize {
    let mut norm_count = 0;
    let mut byte_pos = 0;
    for (i, c) in orig.char_indices() {
        if c.is_ascii_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(&c) {
            norm_count += c.len_utf8(); // to_lowercase 可能变长，但 ASCII 和中文都是 1:1
            if norm_count >= norm_idx {
                byte_pos = i + c.len_utf8();
                break;
            }
        }
        byte_pos = i + c.len_utf8();
    }
    find_char_boundary(orig, byte_pos)
}

/// 简易 YAML frontmatter 解析器（只处理 key: value 行和 [array] 行）
fn parse_simple_yaml(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, val)) = line.split_once(':') {
            let key = key.trim().to_string();
            let val = val.trim().to_string();
            // 去掉引号包裹
            let val = val
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(&val)
                .to_string();
            if !key.is_empty() {
                map.insert(key, val);
            }
        }
    }
    map
}

fn chrono_now() -> String {
    // 不引入 chrono 依赖，用 std 时间
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}s since epoch", d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_patterns_基本() {
        let p = generate_search_patterns("Claude Code 源码泄露后，我反而更确定");
        assert!(!p.is_empty());
        // 第一个应该是完整标题
        assert!(p[0].contains("Claude Code"));
    }

    #[test]
    fn search_patterns_去标点() {
        let p = generate_search_patterns("《测试标题》");
        // 去掉书名号后
        assert!(p.iter().any(|s| s == "测试标题"));
    }

    #[test]
    fn parse_simple_yaml_基本() {
        let yaml = r#"
title: 测试文章
origin: wechat
compiled_to_wiki: true
orphan: true
tags: [AI, Agent]
"#;
        let map = parse_simple_yaml(yaml);
        assert_eq!(map.get("title").unwrap(), "测试文章");
        assert_eq!(map.get("origin").unwrap(), "wechat");
        assert_eq!(map.get("compiled_to_wiki").unwrap(), "true");
    }

    #[test]
    fn audit_category_分类() {
        // C 类：compiled=false
        let orphan_c = OrphanMeta {
            title: "未编译".into(),
            notion_uuid: "abc".into(),
            origin: "x".into(),
            compiled_to_wiki: false,
            url: None,
            tags: vec![],
            relative_path: "sources/x/test.md".into(),
        };
        let wiki_pages = vec![];
        let results = audit_orphans(&[orphan_c], &wiki_pages);
        assert_eq!(results[0].category, AuditCategory::NeverCompiled);

        // B 类：compiled=true 但没匹配到
        let orphan_b = OrphanMeta {
            title: "完全不可能匹配到的标题 XYZ123".into(),
            notion_uuid: "def".into(),
            origin: "wechat".into(),
            compiled_to_wiki: true,
            url: None,
            tags: vec![],
            relative_path: "sources/wechat/test.md".into(),
        };
        let results = audit_orphans(&[orphan_b], &wiki_pages);
        assert_eq!(results[0].category, AuditCategory::CompiledButNotFound);
    }
}
