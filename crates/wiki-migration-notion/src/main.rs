//! CLI 入口：
//!
//! ```bash
//! wiki-migration-notion dry-run \
//!     --wiki /tmp/notion-wiki-clean \
//!     --x    /tmp/notion-db2 \
//!     --wechat /tmp/notion-db3 \
//!     --out /tmp/migration-report.md
//! ```

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use wiki_migration_notion::{audit, model::LibraryKind, report, resolver, scanner, writer};

#[derive(Parser)]
#[command(name = "wiki-migration-notion")]
#[command(about = "Notion 导出 → 本地 Wiki 迁移工具")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// 干跑：扫描 3 个导出目录、解析、输出报告（不写任何 wiki 文件）
    DryRun {
        /// Wiki DB 解压根目录
        #[arg(long)]
        wiki: PathBuf,
        /// X 书签 DB 解压根目录
        #[arg(long)]
        x: PathBuf,
        /// 微信 DB 解压根目录
        #[arg(long)]
        wechat: PathBuf,
        /// 输出报告路径
        #[arg(long, default_value = "migration-report.md")]
        out: PathBuf,
        /// 额外输出 JSONL：pages.jsonl / edges.jsonl / unresolved.jsonl
        #[arg(long)]
        jsonl_dir: Option<PathBuf>,
    },
    /// 真正落盘：把三个库写入本地 wiki 目录（YAML frontmatter + 改写后的正文）
    Migrate {
        #[arg(long)]
        wiki: PathBuf,
        #[arg(long)]
        x: PathBuf,
        #[arg(long)]
        wechat: PathBuf,
        /// 输出根目录（必须为空或不存在）
        #[arg(long, default_value = "/tmp/wiki-migrated")]
        out: PathBuf,
    },
    /// 审计已迁移 vault 中的孤儿 source：标题模糊匹配 + A/B/C 分类
    AuditOrphans {
        /// 已迁移的 vault 根目录（含 pages/ 和 sources/）
        #[arg(long, default_value = "/Users/mac-mini/Documents/wiki")]
        vault: PathBuf,
        /// 审计报告输出路径
        #[arg(long, default_value = "/tmp/orphan-audit-report.md")]
        out: PathBuf,
        /// 同时输出结构化 JSON（包含完整匹配上下文）
        #[arg(long)]
        json: Option<PathBuf>,
    },
    /// 对 A 类孤儿自动补链接：在 Wiki 页正文中插入指向 source 的 Markdown 链接
    FixOrphans {
        /// 已迁移的 vault 根目录
        #[arg(long, default_value = "/Users/mac-mini/Documents/wiki")]
        vault: PathBuf,
        /// 审计 JSON 路径（先运行 audit-orphans --json 生成）
        #[arg(long, default_value = "/tmp/orphan-audit.json")]
        audit_json: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::DryRun {
            wiki,
            x,
            wechat,
            out,
            jsonl_dir,
        } => dry_run(&wiki, &x, &wechat, &out, jsonl_dir.as_deref()),
        Cmd::Migrate {
            wiki,
            x,
            wechat,
            out,
        } => migrate(&wiki, &x, &wechat, &out),
        Cmd::AuditOrphans { vault, out, json } => audit_orphans_cmd(&vault, &out, json.as_deref()),
        Cmd::FixOrphans { vault, audit_json } => fix_orphans_cmd(&vault, &audit_json),
    }
}

fn migrate(
    wiki: &std::path::Path,
    x: &std::path::Path,
    wechat: &std::path::Path,
    out: &std::path::Path,
) -> Result<()> {
    eprintln!("扫描 3 个库...");
    let mut all = scanner::scan_dir(wiki, LibraryKind::Wiki)?;
    all.extend(scanner::scan_dir(x, LibraryKind::XBookmark)?);
    all.extend(scanner::scan_dir(wechat, LibraryKind::WeChat)?);
    eprintln!("  → 共 {} 条", all.len());

    let opts = writer::WriteOptions {
        out_dir: out.to_path_buf(),
        ..Default::default()
    };
    eprintln!("落盘到 {}...", out.display());
    let stats = writer::write_all(&all, &opts)?;
    eprintln!("{}", serde_json::to_string_pretty(&stats)?);
    eprintln!("完成。");
    Ok(())
}

fn dry_run(
    wiki: &std::path::Path,
    x: &std::path::Path,
    wechat: &std::path::Path,
    out: &std::path::Path,
    jsonl_dir: Option<&std::path::Path>,
) -> Result<()> {
    eprintln!("扫描 Wiki...");
    let mut all = scanner::scan_dir(wiki, LibraryKind::Wiki)?;
    eprintln!("  → {} 条", all.len());

    eprintln!("扫描 X书签...");
    let x_pages = scanner::scan_dir(x, LibraryKind::XBookmark)?;
    eprintln!("  → {} 条", x_pages.len());
    all.extend(x_pages);

    eprintln!("扫描 微信...");
    let wc_pages = scanner::scan_dir(wechat, LibraryKind::WeChat)?;
    eprintln!("  → {} 条", wc_pages.len());
    all.extend(wc_pages);

    eprintln!("Resolve 跨库引用...");
    let resolved = resolver::resolve(&all);
    eprintln!(
        "  → 内部边 {} / 外部边 {}+{} / 未解析 {}",
        resolved.stats.internal_resolved,
        resolved.stats.external_resolved_by_url,
        resolved.stats.external_resolved_by_source_url_field,
        resolved.stats.external_unresolved,
    );

    let md = report::render_report(&all, &resolved);
    std::fs::write(out, md)?;
    eprintln!("报告已写入：{}", out.display());

    if let Some(dir) = jsonl_dir {
        std::fs::create_dir_all(dir)?;
        write_jsonl(&dir.join("pages.jsonl"), &all)?;
        write_jsonl(&dir.join("edges.jsonl"), &resolved.edges)?;
        write_jsonl(&dir.join("unresolved.jsonl"), &resolved.unresolved)?;
        eprintln!("JSONL 已写入：{}", dir.display());
    }
    Ok(())
}

fn write_jsonl<T: serde::Serialize>(path: &std::path::Path, items: &[T]) -> Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    for item in items {
        let line = serde_json::to_string(item)?;
        f.write_all(line.as_bytes())?;
        f.write_all(b"\n")?;
    }
    Ok(())
}

fn audit_orphans_cmd(
    vault: &std::path::Path,
    out: &std::path::Path,
    json_path: Option<&std::path::Path>,
) -> Result<()> {
    eprintln!("扫描孤儿 source...");
    let orphans = audit::scan_orphan_sources(vault)?;
    eprintln!("  → 找到 {} 条孤儿 source", orphans.len());

    eprintln!("扫描 Wiki 页面正文...");
    let wiki_pages = audit::scan_wiki_pages(vault)?;
    eprintln!("  → 找到 {} 个 Wiki 页面", wiki_pages.len());

    eprintln!("执行标题模糊匹配...");
    let entries = audit::audit_orphans(&orphans, &wiki_pages);

    let stats = audit::compute_stats(&entries);
    eprintln!(
        "  → A(标题匹配): {}  B(已编译未匹配): {}  C(未编译): {}",
        stats.category_a, stats.category_b, stats.category_c
    );

    // 渲染 Markdown 报告
    let report = audit::render_report(&entries, &stats);
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out, &report)?;
    eprintln!("报告已写入：{}", out.display());

    // 可选：输出结构化 JSON
    if let Some(json_path) = json_path {
        let output = serde_json::json!({
            "stats": stats,
            "entries": entries,
        });
        if let Some(parent) = json_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(json_path, serde_json::to_string_pretty(&output)?)?;
        eprintln!("JSON 已写入：{}", json_path.display());
    }

    Ok(())
}

fn fix_orphans_cmd(vault: &std::path::Path, audit_json: &std::path::Path) -> Result<()> {
    eprintln!("读取审计 JSON: {}...", audit_json.display());
    let stats = audit::fix_orphans(vault, audit_json)?;
    eprintln!("{}", serde_json::to_string_pretty(&stats)?);
    eprintln!(
        "完成：修改了 {} 个 Wiki 页面，插入了 {} 条 source 链接",
        stats.wiki_pages_modified, stats.links_inserted
    );
    Ok(())
}
