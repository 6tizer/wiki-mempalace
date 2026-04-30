//! LLM Wiki v2 用例编排：内存参考引擎 + 事件钩子（可接外部记忆系统）。

mod auto_hooks;
mod engine;
mod evidence_fixer_apply;
mod evidence_fixer_plan;
mod fix;
mod gap;
mod governance_scan;
mod hooks;
mod memory;
mod metrics;
mod search_ports;
mod strategy;
mod synthesis_discovery;
mod wiki_writer;

pub mod page_contract;
pub use auto_hooks::AutoWikiHook;
pub use engine::{
    collect_basic_lint_findings, initial_status_for, EngineError, LlmWikiEngine, PromotePageError,
};
pub use evidence_fixer_apply::{
    apply_evidence_fixer_plan, restore_evidence_fixer_tombstone, EvidenceFixerApplyOptions,
};
pub use evidence_fixer_plan::{
    build_evidence_fixer_plan, duplicate_web_verification_key, EvidenceFixerPlanOptions,
};
pub use fix::{map_findings_to_fixes, map_gap_finding, map_lint_finding};
pub use gap::run_gap_scan;
pub use governance_scan::{run_governance_scan, GovernanceScanOptions};
pub use hooks::{NoopWikiHook, WikiHook};
pub use memory::InMemoryStore;
pub use metrics::collect_wiki_metrics;
pub use page_contract::finalize_consumed_page;
pub use search_ports::{
    format_claim_doc_id, format_entity_doc_id, format_page_doc_id, merge_graph_rankings,
    EmptySearchPorts, InMemorySearchPorts, SearchPorts,
};
pub use strategy::{run_strategy_scan, StrategyScanOptions};
pub use synthesis_discovery::{discover_synthesis_candidates, SynthesisDiscoveryOptions};
pub use wiki_writer::{
    write_lint_report, write_projection, write_projection_pages, ProjectionStats,
};
