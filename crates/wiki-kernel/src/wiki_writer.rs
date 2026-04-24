use crate::InMemoryStore;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io;
use std::path::Path;
use uuid::Uuid;
use wiki_core::{AuditRecord, Claim, LintFinding, LintSeverity, RawArtifact, Scope, WikiPage};

#[derive(Debug, Default, Clone, Copy)]
pub struct ProjectionStats {
    pub pages_written: usize,
    pub claims_written: usize,
    pub sources_written: usize,
}

pub fn write_projection(
    wiki_root: &Path,
    store: &InMemoryStore,
    audits: &[AuditRecord],
) -> io::Result<ProjectionStats> {
    let pages_dir = wiki_root.join("pages");
    fs::create_dir_all(&pages_dir)?;
    cleanup_stale_managed_pages(&pages_dir, store)?;

    let mut stats = ProjectionStats::default();
    let mut page_rows = Vec::new();
    // 不再向根 `concepts/` 写哈希 claim 投影；root 仅保留 Notion 对齐的目录
    // （见 docs/vault-standards.md）
    let claim_rows: Vec<String> = Vec::new();
    // 不在 `sources/` 根目录写引擎投影；source 文件由迁移/抓取工具维护（见 vault-standards）
    let source_rows: Vec<String> = Vec::new();

    let mut pages: Vec<_> = store.pages.values().collect();
    pages.sort_by(|a, b| a.title.cmp(&b.title).then_with(|| a.id.0.cmp(&b.id.0)));
    for page in pages {
        let subdir = page_subdir_for_entry_type(page.entry_type.as_ref());
        let dir = pages_dir.join(subdir);
        fs::create_dir_all(&dir)?;
        let fname = vault_page_filename(&page.title);
        let path = dir.join(format!("{fname}.md"));
        fs::write(&path, render_page_with_frontmatter(page))?;
        stats.pages_written += 1;
        let rel = format!("pages/{subdir}/{fname}.md");
        page_rows.push(format!(
            "- [{}]({}) | updated: {}",
            page.title,
            rel,
            page.updated_at.date()
        ));
    }

    // Claim 不再作为独立 markdown 文件落盘；其语义由 page（`pages/concept/` 等）承载。
    // `render_claim_with_frontmatter` 仍保留供测试 / 未来导出使用。
    let _ = &store.claims;

    fs::write(
        wiki_root.join("index.md"),
        render_index(&page_rows, &claim_rows, &source_rows),
    )?;
    fs::write(wiki_root.join("log.md"), render_log(audits))?;
    Ok(stats)
}

fn cleanup_stale_managed_pages(pages_dir: &Path, store: &InMemoryStore) -> io::Result<()> {
    let current_page_ids: HashSet<Uuid> = store.pages.keys().map(|id| id.0).collect();
    for path in markdown_files_under(pages_dir)? {
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(err) if err.kind() == io::ErrorKind::InvalidData => continue,
            Err(err) => return Err(err),
        };
        let Some(id) = managed_frontmatter_page_id(&content) else {
            continue;
        };
        if !current_page_ids.contains(&id) {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn markdown_files_under(root: &Path) -> io::Result<Vec<std::path::PathBuf>> {
    let mut out = Vec::new();
    if !root.exists() {
        return Ok(out);
    }
    collect_markdown_files(root, &mut out)?;
    Ok(out)
}

fn collect_markdown_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let ft = entry.file_type()?;
        if ft.is_dir() {
            collect_markdown_files(&path, out)?;
        } else if ft.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
        }
    }
    Ok(())
}

fn managed_frontmatter_page_id(content: &str) -> Option<Uuid> {
    let rest = content.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    let frontmatter = &rest[..end];
    for line in frontmatter.lines() {
        let trimmed = line.trim();
        let Some(raw_id) = trimmed.strip_prefix("id:") else {
            continue;
        };
        let raw_id = raw_id.trim().trim_matches('"').trim_matches('\'');
        return Uuid::parse_str(raw_id).ok();
    }
    None
}

pub fn write_lint_report(
    wiki_root: &Path,
    report_name: &str,
    findings: &[LintFinding],
) -> io::Result<std::path::PathBuf> {
    let reports_dir = wiki_root.join("reports");
    fs::create_dir_all(&reports_dir)?;
    let filename = if report_name.ends_with(".md") {
        report_name.to_string()
    } else {
        format!("{report_name}.md")
    };
    let out = reports_dir.join(filename);
    let mut grouped: BTreeMap<&'static str, Vec<&LintFinding>> = BTreeMap::new();
    for f in findings {
        let key = match f.severity {
            LintSeverity::Error => "error",
            LintSeverity::Warn => "warn",
            LintSeverity::Info => "info",
        };
        grouped.entry(key).or_default().push(f);
    }
    let mut md = String::from("# Lint Report\n\n");
    md.push_str(&format!("- total findings: `{}`\n\n", findings.len()));
    for (k, items) in grouped {
        md.push_str(&format!("## {}\n\n", k));
        for item in items {
            md.push_str(&format!(
                "- `{}` {}{}\n",
                item.code,
                item.message,
                item.subject
                    .as_ref()
                    .map(|s| format!(" (subject={s})"))
                    .unwrap_or_default()
            ));
        }
        md.push('\n');
    }
    fs::write(&out, md)?;
    Ok(out)
}

/// EntryStatus → snake_case 字符串（与 serde rename_all 保持一致）。
fn status_str(s: wiki_core::EntryStatus) -> &'static str {
    match s {
        wiki_core::EntryStatus::Draft => "draft",
        wiki_core::EntryStatus::InReview => "in_review",
        wiki_core::EntryStatus::Approved => "approved",
        wiki_core::EntryStatus::NeedsUpdate => "needs_update",
    }
}

/// EntryType → snake_case 字符串（与 serde rename_all 保持一致）。
fn entry_type_str(t: &wiki_core::EntryType) -> &'static str {
    match t {
        wiki_core::EntryType::Concept => "concept",
        wiki_core::EntryType::Entity => "entity",
        wiki_core::EntryType::Summary => "summary",
        wiki_core::EntryType::Synthesis => "synthesis",
        wiki_core::EntryType::Qa => "qa",
        wiki_core::EntryType::LintReport => "lint_report",
        wiki_core::EntryType::Index => "index",
    }
}

/// 按 `entry_type` 映射到 `pages/` 下子目录（与 docs/vault-standards.md 一致）。
fn page_subdir_for_entry_type(et: Option<&wiki_core::EntryType>) -> &'static str {
    match et {
        Some(wiki_core::EntryType::Summary) => "summary",
        Some(wiki_core::EntryType::Concept) => "concept",
        Some(wiki_core::EntryType::Entity) => "entity",
        Some(wiki_core::EntryType::Synthesis) => "synthesis",
        Some(wiki_core::EntryType::Qa) => "qa",
        Some(wiki_core::EntryType::LintReport) => "lint-report",
        Some(wiki_core::EntryType::Index) => "index",
        None => "_unspecified",
    }
}

/// vault 命名：中文标题直用，仅将 `/` 替换为 `-`（不做 ASCII slugify）。
fn vault_page_filename(title: &str) -> String {
    title.replace('/', "-")
}

/// YAML 双引号内转义：只处理双引号和反斜杠。
fn yaml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn render_yaml_string_list(field: &str, values: &[String]) -> String {
    if values.is_empty() {
        return format!("{field}: []\n");
    }
    let mut out = format!("{field}:\n");
    for value in values {
        out.push_str(&format!("  - \"{}\"\n", yaml_escape(value)));
    }
    out
}

/// 将 Scope 序列化为人类可读字符串（用于 YAML frontmatter）。
fn scope_label(scope: &Scope) -> String {
    match scope {
        Scope::Private { agent_id } => format!("private:{agent_id}"),
        Scope::Shared { team_id } => format!("shared:{team_id}"),
    }
}

/// 渲染 WikiPage 为带 YAML frontmatter 的完整 Markdown。
fn render_page_with_frontmatter(page: &WikiPage) -> String {
    let mut fm = String::from("---\n");
    fm.push_str(&format!("id: \"{}\"\n", page.id.0));
    fm.push_str(&format!("title: \"{}\"\n", yaml_escape(&page.title)));
    fm.push_str(&format!("status: {}\n", status_str(page.status)));
    match &page.entry_type {
        Some(et) => {
            fm.push_str(&format!("entry_type: {}\n", entry_type_str(et)));
        }
        None => fm.push_str("entry_type: null\n"),
    }
    fm.push_str(&format!(
        "scope: \"{}\"\n",
        yaml_escape(&scope_label(&page.scope))
    ));
    fm.push_str(&format!("updated_at: {}\n", page.updated_at.date()));
    fm.push_str("---\n\n");
    fm.push_str(&page.markdown);
    fm
}

/// 渲染 Claim 为带 YAML frontmatter 的完整 Markdown。
///
/// vault-standards 对齐后，`write_projection` 不再向 `concepts/` 写哈希命名的 claim 文件；
/// 本函数保留供测试与未来的显式导出使用。
#[allow(dead_code)]
fn render_claim_with_frontmatter(claim: &Claim) -> String {
    let short = claim.id.0.to_string();
    let id_short = &short[..8];
    let mut fm = String::from("---\n");
    fm.push_str(&format!("id: \"{}\"\n", claim.id.0));
    fm.push_str(&format!("tier: {:?}\n", claim.tier));
    fm.push_str(&format!("confidence: {:.3}\n", claim.confidence));
    fm.push_str(&format!("quality: {:.3}\n", claim.quality_score));
    fm.push_str(&format!("stale: {}\n", claim.stale));
    fm.push_str(&format!("sources_count: {}\n", claim.source_ids.len()));
    fm.push_str(&render_yaml_string_list("tags", &claim.tags));
    fm.push_str(&format!("created_at: {}\n", claim.created_at.date()));
    fm.push_str("---\n\n");
    // 正文保持原有结构
    fm.push_str(&format!(
        "# Claim {id_short}\n\n- tier: `{:?}`\n- confidence: `{:.3}`\n- quality: `{:.3}`\n- stale: `{}`\n- sources: `{}`\n\n## Text\n\n{}\n",
        claim.tier,
        claim.confidence,
        claim.quality_score,
        claim.stale,
        claim.source_ids.len(),
        claim.text
    ));
    fm
}

/// 渲染 RawArtifact (Source) 为带 YAML frontmatter 的完整 Markdown。
///
/// 自 vault-standards 对齐起，`write_projection` 不再写入 `sources/` 根目录，
/// 此函数仅保留供测试与潜在外部调用使用（见 [docs/vault-standards.md]）。
#[allow(dead_code)]
fn render_source_with_frontmatter(source: &RawArtifact) -> String {
    let short = source.id.0.to_string();
    let id_short = &short[..8];
    let preview = preview_text(&source.body, 2000);
    let mut fm = String::from("---\n");
    fm.push_str(&format!("id: \"{}\"\n", source.id.0));
    fm.push_str(&format!("uri: \"{}\"\n", yaml_escape(&source.uri)));
    fm.push_str(&render_yaml_string_list("tags", &source.tags));
    fm.push_str(&format!("ingested_at: {}\n", source.ingested_at.date()));
    fm.push_str("---\n\n");
    fm.push_str(&format!(
        "# Source {id_short}\n\n- uri: `{}`\n- ingested_at: `{}`\n\n## Preview\n\n{}\n",
        source.uri, source.ingested_at, preview
    ));
    fm
}

fn render_index(pages: &[String], claims: &[String], sources: &[String]) -> String {
    let mut md = String::from("# index\n\n");
    md.push_str("## pages\n\n");
    if pages.is_empty() {
        md.push_str("- (empty)\n");
    } else {
        for l in pages {
            md.push_str(l);
            md.push('\n');
        }
    }
    md.push_str("\n## concepts\n\n");
    if claims.is_empty() {
        md.push_str("- (empty)\n");
    } else {
        for l in claims {
            md.push_str(l);
            md.push('\n');
        }
    }
    md.push_str("\n## sources\n\n");
    if sources.is_empty() {
        md.push_str("- (empty)\n");
    } else {
        for l in sources {
            md.push_str(l);
            md.push('\n');
        }
    }
    md
}

fn render_log(audits: &[AuditRecord]) -> String {
    let mut lines = String::from("# log\n\n");
    let mut rows: Vec<_> = audits.iter().collect();
    rows.sort_by_key(|a| a.at);
    for a in rows {
        lines.push_str(&format!(
            "## [{}] {:?} | {}\n- actor: `{}`\n- summary: {}\n\n",
            a.at, a.op, a.id, a.actor, a.summary
        ));
    }
    lines
}

#[allow(dead_code)]
fn preview_text(body: &str, max_len: usize) -> String {
    let mut s = body.to_string();
    if s.len() > max_len {
        // 字符安全截断：找到 <= max_len 的最大字符边界
        let boundary = s
            .char_indices()
            .take_while(|(byte_idx, _)| *byte_idx < max_len)
            .last()
            .map(|(byte_idx, c)| byte_idx + c.len_utf8())
            .unwrap_or(0);
        s.truncate(boundary);
        s.push_str("\n\n...truncated...");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use wiki_core::{
        schema::{EntryStatus, EntryType},
        AuditOperation, AuditRecord, Claim, MemoryTier, RawArtifact, Scope, WikiPage,
    };

    fn private_scope() -> Scope {
        Scope::Private {
            agent_id: "test".into(),
        }
    }

    #[test]
    fn projection_writes_index_log_and_dirs() {
        let dir = tempdir().unwrap();
        let wiki_root = dir.path();

        let mut store = InMemoryStore::default();
        let scope = private_scope();

        let src = RawArtifact::new("file:///a.md", "hello world", scope.clone());
        store.sources.insert(src.id, src);

        let claim = Claim::new("Redis caching for API", scope.clone(), MemoryTier::Semantic);
        store.claims.insert(claim.id, claim);

        let page = WikiPage::new("Alpha Page", "Link to [[Beta Page]]", scope.clone());
        store.pages.insert(page.id, page);

        let audits = vec![AuditRecord::new(AuditOperation::IngestSource, "t", "x")];
        let stats = write_projection(wiki_root, &store, &audits).unwrap();
        assert!(stats.pages_written >= 1);
        assert!(wiki_root.join("index.md").exists());
        assert!(wiki_root.join("log.md").exists());
        assert!(wiki_root.join("pages").is_dir());
        // vault-standards：不再向根 `concepts/` 写哈希 claim 投影
        assert!(
            !wiki_root.join("concepts").exists()
                || std::fs::read_dir(wiki_root.join("concepts"))
                    .unwrap()
                    .next()
                    .is_none(),
            "projection 不应在根 concepts/ 写入文件"
        );
        // vault-standards：不再向 `sources/` 根目录写引擎投影
        assert!(
            !wiki_root.join("sources").join("any.md").exists(),
            "projection 不应写入 sources/"
        );

        // pages 已按 entry_type 分子目录；无 entry_type 落到 `_unspecified/`
        let pages_root = wiki_root.join("pages");
        let mut page_files: Vec<std::path::PathBuf> = Vec::new();
        for sub in std::fs::read_dir(&pages_root).unwrap().flatten() {
            if sub.path().is_dir() {
                for f in std::fs::read_dir(sub.path()).unwrap().flatten() {
                    if f.path().extension().is_some_and(|x| x == "md") {
                        page_files.push(f.path());
                    }
                }
            }
        }
        assert!(!page_files.is_empty(), "pages/ 子目录应有 md 文件");
        for p in &page_files {
            let content = std::fs::read_to_string(p).unwrap();
            assert!(
                content.starts_with("---\n"),
                "page 文件应以 frontmatter 开头: {:?}",
                p
            );
            assert!(
                content.contains("status:"),
                "page 文件 frontmatter 应含 status 字段"
            );
        }
    }

    #[test]
    fn projection_pages_split_by_entry_type() {
        let dir = tempdir().unwrap();
        let wiki_root = dir.path();
        let mut store = InMemoryStore::default();
        let sum = WikiPage::new("标题/含斜杠", "body", private_scope())
            .with_entry_type(EntryType::Summary);
        let con =
            WikiPage::new("概念页", "body", private_scope()).with_entry_type(EntryType::Concept);
        store.pages.insert(sum.id, sum);
        store.pages.insert(con.id, con);
        write_projection(wiki_root, &store, &[]).unwrap();
        // 中文标题直用，`/` → `-`
        assert!(wiki_root
            .join("pages")
            .join("summary")
            .join("标题-含斜杠.md")
            .exists());
        assert!(wiki_root
            .join("pages")
            .join("concept")
            .join("概念页.md")
            .exists());
    }

    #[test]
    fn projection_removes_stale_managed_page_and_preserves_unmanaged() {
        let dir = tempdir().unwrap();
        let wiki_root = dir.path();
        let pages_dir = wiki_root.join("pages").join("concept");
        std::fs::create_dir_all(&pages_dir).unwrap();

        let stale_id = uuid::Uuid::new_v4();
        let stale = pages_dir.join("stale.md");
        std::fs::write(
            &stale,
            format!(
                "---\nid: \"{}\"\ntitle: \"Stale\"\n---\n\n# stale\n",
                stale_id
            ),
        )
        .unwrap();

        let unmanaged = pages_dir.join("unmanaged.md");
        std::fs::write(&unmanaged, "# unmanaged\n").unwrap();

        let invalid_id = pages_dir.join("invalid-id.md");
        std::fs::write(&invalid_id, "---\nid: \"not-a-uuid\"\n---\n\n# invalid\n").unwrap();

        let mut store = InMemoryStore::default();
        let page =
            WikiPage::new("Current", "body", private_scope()).with_entry_type(EntryType::Concept);
        store.pages.insert(page.id, page);

        write_projection(wiki_root, &store, &[]).unwrap();

        assert!(!stale.exists(), "stale managed page should be removed");
        assert!(unmanaged.exists(), "unmanaged markdown should be preserved");
        assert!(
            invalid_id.exists(),
            "markdown with invalid managed id should be preserved"
        );
        assert!(pages_dir.join("Current.md").exists());
    }

    // --- D1 frontmatter 测试 ---

    #[test]
    fn frontmatter_page_with_status_and_type() {
        let page = WikiPage::new("Test Page", "# body", private_scope())
            .with_entry_type(EntryType::Concept)
            .with_status(EntryStatus::Approved);
        let rendered = render_page_with_frontmatter(&page);
        assert!(rendered.starts_with("---\n"));
        assert!(rendered.contains("status: approved\n"));
        assert!(rendered.contains("entry_type: concept\n"));
        assert!(rendered.contains(&format!("id: \"{}\"\n", page.id.0)));
        assert!(rendered.contains("title: \"Test Page\"\n"));
    }

    #[test]
    fn frontmatter_page_entry_type_null_when_absent() {
        let page = WikiPage::new("NoType", "body", private_scope());
        let rendered = render_page_with_frontmatter(&page);
        assert!(rendered.contains("entry_type: null\n"));
        assert!(rendered.contains("status: draft\n"));
    }

    #[test]
    fn frontmatter_page_preserves_body_verbatim() {
        let body = "# Heading\n\nSome **bold** text\n\n- item 1\n- item 2\n";
        let page = WikiPage::new("BodyTest", body, private_scope());
        let rendered = render_page_with_frontmatter(&page);
        // frontmatter 结束后，body 应逐字符保留
        let body_start = rendered.find("---\n\n").unwrap() + 5;
        assert_eq!(&rendered[body_start..], body);
    }

    #[test]
    fn frontmatter_claim_contains_tier_and_confidence() {
        let mut claim = Claim::new("test claim", private_scope(), MemoryTier::Semantic);
        claim.tags = vec!["alpha".into(), "quoted \"tag\"".into()];
        let rendered = render_claim_with_frontmatter(&claim);
        assert!(rendered.starts_with("---\n"));
        assert!(rendered.contains(&format!("id: \"{}\"\n", claim.id.0)));
        assert!(rendered.contains("tier: Semantic\n"));
        assert!(rendered.contains("confidence:"));
        assert!(rendered.contains("quality:"));
        assert!(rendered.contains("stale: false\n"));
        assert!(rendered.contains("sources_count: 0\n"));
        assert!(rendered.contains("tags:\n  - \"alpha\"\n  - \"quoted \\\"tag\\\"\"\n"));
    }

    #[test]
    fn frontmatter_source_contains_uri() {
        let source = RawArtifact::new("file:///notes/test.md", "body text", private_scope())
            .with_tags(["alpha", "beta"]);
        let rendered = render_source_with_frontmatter(&source);
        assert!(rendered.starts_with("---\n"));
        assert!(rendered.contains(&format!("id: \"{}\"\n", source.id.0)));
        assert!(rendered.contains("uri: \"file:///notes/test.md\"\n"));
        assert!(rendered.contains("tags:\n  - \"alpha\"\n  - \"beta\"\n"));
        assert!(rendered.contains("ingested_at:"));
        assert!(rendered.contains("## Preview"));
    }

    #[test]
    fn frontmatter_idempotent_on_rewrite() {
        let page =
            WikiPage::new("Idempotent", "body", private_scope()).with_status(EntryStatus::InReview);
        let first = render_page_with_frontmatter(&page);
        let second = render_page_with_frontmatter(&page);
        assert_eq!(first, second, "两次渲染输出应字节级一致");
    }

    #[test]
    fn frontmatter_title_with_quotes_escapes_properly() {
        let page = WikiPage::new("He said \"hello\" then left", "body", private_scope());
        let rendered = render_page_with_frontmatter(&page);
        assert!(rendered.contains(r#"title: "He said \"hello\" then left""#));
    }
}
