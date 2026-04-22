//! 递归扫描单个导出目录，产出 `Vec<RawPage>`。

use crate::model::{LibraryKind, RawPage};
use crate::parser;
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// 扫一个 Notion 导出目录（ditto 解压后的 `私人与共享/...` 根即可）
pub fn scan_dir(dir: &Path, library: LibraryKind) -> Result<Vec<RawPage>> {
    if !dir.exists() {
        bail!("目录不存在: {}", dir.display());
    }

    // 说明：我们仅处理 .md；CSV 当索引忽略（信息在 md 里都有），.zip 和子目录里的
    // 顶层 index 页（如 `Wiki Index（导航地图）.md`）也会被扫进来，交给后续过滤。
    let mut out = Vec::new();
    let mut skipped: Vec<(PathBuf, String)> = Vec::new();

    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
        if ext != "md" {
            continue;
        }
        match parser::parse_file(p, library) {
            Ok(page) => out.push(page),
            Err(e) => skipped.push((p.to_path_buf(), format!("{e:#}"))),
        }
    }

    if !skipped.is_empty() {
        eprintln!(
            "[{}] 跳过 {} 个文件（通常是没有 UUID 的 index 页）：",
            library.as_str(),
            skipped.len()
        );
        for (p, why) in skipped.iter().take(5) {
            eprintln!("  - {} : {}", p.display(), why);
        }
        if skipped.len() > 5 {
            eprintln!("  ... ({} 个省略)", skipped.len() - 5);
        }
    }

    Ok(out)
}
