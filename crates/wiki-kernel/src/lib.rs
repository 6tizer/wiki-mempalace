//! LLM Wiki v2 用例编排：内存参考引擎 + 事件钩子（可接外部记忆系统）。

mod auto_hooks;
mod engine;
mod fix;
mod gap;
mod hooks;
mod memory;
mod search_ports;
mod wiki_writer;

pub use auto_hooks::AutoWikiHook;
pub use engine::{initial_status_for, EngineError, LlmWikiEngine, PromotePageError};
pub use fix::{map_findings_to_fixes, map_gap_finding, map_lint_finding};
pub use gap::run_gap_scan;
pub use hooks::{NoopWikiHook, WikiHook};
pub use memory::InMemoryStore;
pub use search_ports::{
    format_claim_doc_id, format_entity_doc_id, format_page_doc_id, merge_graph_rankings,
    EmptySearchPorts, InMemorySearchPorts, SearchPorts,
};
pub use wiki_writer::{write_lint_report, write_projection, ProjectionStats};
