//! LLM Wiki v2 核心理念的领域模型与纯函数策略（无 IO）。
//!
//! 对应 [rohitg00 / LLM Wiki v2](https://gist.github.com/rohitg00/2067ab416f7bbe447c1977edaaa681e2)：
//! 原始资料 / Wiki 页 / Schema；记忆生命周期；类型化知识图；混合检索 RRF；事件与审计；
//! 质量与矛盾；协作与隐私；结晶输出草稿。

pub mod artifact;
pub mod audit;
pub mod collab;
pub mod crystallize;
pub mod events;
pub mod fix;
pub mod gap;
pub mod graph;
pub mod lifecycle;
pub mod llm_ingest_plan;
pub mod model;
pub mod page;
pub mod privacy;
pub mod quality;
pub mod query;
pub mod retention;
pub mod schema;
pub mod scope_policy;
pub mod search;
pub mod search_ports;

pub use artifact::RawArtifact;
pub use audit::{AuditOperation, AuditRecord};
pub use collab::{WorkItem, WorkState};
pub use crystallize::{draft_from_session, CrystallizationDraft, SessionCrystallizationInput};
pub use events::WikiEvent;
pub use fix::{FixAction, FixActionType, FixPatch};
pub use gap::{GapFinding, GapSeverity};
pub use graph::{walk_entities, GraphSnapshot, GraphWalkOptions};
pub use lifecycle::{
    advance_tier, apply_time_decay_to_confidence, merge_sources_confidence, reinforce_claim,
    supersede_claim,
};
pub use llm_ingest_plan::{
    parse_memory_tier, LlmClaimDraft, LlmEntityDraft, LlmIngestPlanV1, LlmRelationDraft,
};
pub use model::{
    Claim, ClaimId, Entity, EntityId, EntityKind, MemoryTier, PageId, RelationKind, Scope,
    SourceId, TypedEdge,
};
pub mod page_contract;
pub use page::{
    extract_headings, extract_wikilinks, Confidence, WikiPage,
};
pub use page_contract::PageContract;
pub use privacy::{redact_for_ingest, RedactionFinding, SensitiveKind};
pub use quality::{check_page_completeness, ContradictionHint, LintFinding, LintSeverity};
pub use query::QueryContext;
pub use retention::{retention_strength, RetentionParams};
pub use schema::{
    CompletenessConfig, DomainSchema, EntryStatus, EntryType, LifecycleRule, PromotionConditions,
    PromotionRule, SchemaLoadError, SchemaValidationError, TagConfig, DEFAULT_MAINTENANCE_BATCH,
};
pub use scope_policy::document_visible_to_viewer;
pub use search::{reciprocal_rank_fusion, RankedDoc};
pub use search_ports::SearchPorts;
