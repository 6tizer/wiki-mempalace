//! Notion 导出 → 本地 Wiki 的一次性迁移工具库。
//!
//! 本 crate 只读本地磁盘（"离线 parser"），不调 Notion API。
//! 入口：`bin/migrate.rs`（CLI）。

pub mod audit;
pub mod model;
pub mod parser;
pub mod report;
pub mod resolver;
pub mod scanner;
pub mod writer;
