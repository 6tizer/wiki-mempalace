//! 扫描单个 Notion 导出目录，产出 `Vec<RawPage>`。

use crate::parser;
use crate::{
    model::{LibraryKind, RawPage},
    resolver::get_property,
};
use anyhow::{bail, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
pub struct ScanOutput {
    pub pages: Vec<RawPage>,
    pub skipped: Vec<ScanSkip>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanSkip {
    pub library: LibraryKind,
    pub path: PathBuf,
    pub reason: String,
}

/// 扫一个 Notion 导出目录（ditto 解压后的 `私人与共享/...` 根即可）
pub fn scan_dir(dir: &Path, library: LibraryKind) -> Result<Vec<RawPage>> {
    Ok(scan_dir_with_report(dir, library)?.pages)
}

pub fn scan_dir_with_report(dir: &Path, library: LibraryKind) -> Result<ScanOutput> {
    if !dir.exists() {
        bail!("目录不存在: {}", dir.display());
    }

    let scan_root = find_export_table_root(dir, library).unwrap_or_else(|| dir.to_path_buf());

    let mut out = Vec::new();
    let mut skipped: Vec<ScanSkip> = Vec::new();

    for entry in WalkDir::new(&scan_root)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
        if ext != "md" {
            continue;
        }
        match parser::parse_file(p, library) {
            Ok(page) => {
                if should_keep_page(&page) {
                    out.push(page);
                } else {
                    skipped.push(ScanSkip {
                        library,
                        path: p.to_path_buf(),
                        reason: "非正式数据库条目".to_string(),
                    });
                }
            }
            Err(e) => skipped.push(ScanSkip {
                library,
                path: p.to_path_buf(),
                reason: format!("{e:#}"),
            }),
        }
    }

    if !skipped.is_empty() {
        eprintln!(
            "[{}] 扫描根 {}；跳过 {} 个文件：",
            library.as_str(),
            scan_root.display(),
            skipped.len()
        );
        for skip in skipped.iter().take(5) {
            eprintln!("  - {} : {}", skip.path.display(), skip.reason);
        }
        if skipped.len() > 5 {
            eprintln!("  ... ({} 个省略)", skipped.len() - 5);
        }
    }

    Ok(ScanOutput {
        pages: out,
        skipped,
    })
}

fn find_export_table_root(dir: &Path, library: LibraryKind) -> Option<PathBuf> {
    let target = match library {
        LibraryKind::Wiki => "知识 Wiki",
        LibraryKind::XBookmark => "X书签文章数据库",
        LibraryKind::WeChat => "文章数据库",
    };
    if dir.file_name().and_then(|s| s.to_str()) == Some(target) {
        return Some(dir.to_path_buf());
    }

    let mut candidates = Vec::new();
    for entry in WalkDir::new(dir)
        .max_depth(5)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_dir() || path == dir {
            continue;
        }
        if path.file_name().and_then(|s| s.to_str()) == Some(target) {
            if matches!(library, LibraryKind::WeChat)
                && path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|s| s.to_str())
                    != Some("微信文章数据库")
            {
                continue;
            }
            candidates.push(path.to_path_buf());
        }
    }
    candidates.sort();
    candidates.into_iter().next()
}

fn should_keep_page(page: &RawPage) -> bool {
    match page.library {
        LibraryKind::Wiki => get_property(page, "类型")
            .and_then(|raw| wiki_core::schema::EntryType::parse(raw).ok())
            .is_some(),
        LibraryKind::XBookmark => true,
        LibraryKind::WeChat => {
            get_property(page, "来源").is_some() || get_property(page, "文章链接").is_some()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{name}-{}-{nanos}", std::process::id()))
    }

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    #[test]
    fn wiki_scan_uses_direct_database_children_only() {
        let root = unique_temp_dir("wiki-migration-scan-wiki");
        let db = root.join("私人与共享/知识 Wiki");
        write(
            &db.join("Concept 11111111111111111111111111111111.md"),
            "# Concept\n\n类型: concept\n状态: 草稿\n\nbody",
        );
        write(
            &db.join("Nested/Task 22222222222222222222222222222222.md"),
            "# Task\n\n类型: concept\n状态: 草稿\n\nbody",
        );
        write(
            &db.join("No Type 33333333333333333333333333333333.md"),
            "# No Type\n\n状态: 草稿\n\nbody",
        );

        let pages = scan_dir(&root, LibraryKind::Wiki).unwrap();

        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].title, "Concept");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn wechat_scan_skips_root_nested_and_invalid_direct_pages() {
        let root = unique_temp_dir("wiki-migration-scan-wechat");
        let db = root.join("私人与共享/微信文章数据库/文章数据库");
        write(
            &db.join("Valid 11111111111111111111111111111111.md"),
            "# Valid\n\n来源: 微信\n文章链接: https://example.test/a\n\nbody",
        );
        write(
            &db.join("无标题 22222222222222222222222222222222.md"),
            "# 无标题\n\n已编译到Wiki: No\n\nbody",
        );
        write(
            &db.join("Nested/Valid 33333333333333333333333333333333.md"),
            "# Valid Nested\n\n来源: 微信\n文章链接: https://example.test/b\n\nbody",
        );
        write(
            &root.join("私人与共享/微信文章数据库 44444444444444444444444444444444.md"),
            "# Root\n\nbody",
        );

        let pages = scan_dir(&root, LibraryKind::WeChat).unwrap();

        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].title, "Valid");
        std::fs::remove_dir_all(root).unwrap();
    }
}
