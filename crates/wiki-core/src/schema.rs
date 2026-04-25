//! Wiki 领域 Schema：类型定义、晋升规则、标签体系、生命周期参数。
//! 本文件是系统的「宪法」，所有自动化规则都从这里读取。
//!
//! 设计约定：
//!
//! - `DomainSchema::from_json_*` 在反序列化后会自动执行 [`DomainSchema::validate`]，
//!   任何语义错误（EntryType 重复、promotion 自环或成环）都会在加载阶段失败，
//!   避免运行时才暴露问题。
//! - `PromotionConditions::required_sections` 表示 **仅在本次晋升中额外要求** 的段落，
//!   `CompletenessConfig::*_required_sections` 表示 **lint 基线**（无论是否晋升都要检查），
//!   二者职责独立，不互相覆盖。

use crate::model::{EntityKind, MemoryTier, RelationKind};
use crate::retention::RetentionParams;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// 维护命令的默认批处理大小，常量化以避免 `default_batch_size` 与 `permissive_default` 漂移。
pub const DEFAULT_MAINTENANCE_BATCH: u32 = 128;

#[derive(Debug, thiserror::Error)]
pub enum SchemaLoadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("schema invalid: {0}")]
    Invalid(#[from] SchemaValidationError),
}

/// Schema 语义级错误：结构能反序列化但不自洽。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SchemaValidationError {
    #[error("EntryType {0:?} 在多条 lifecycle_rules 中重复出现")]
    DuplicateEntryType(EntryType),
    #[error("lifecycle 规则包含自环晋升：{0:?} -> {0:?}")]
    SelfLoopPromotion(EntryStatus),
    #[error("lifecycle 规则 promotions 形成环路")]
    PromotionCycle,
    #[error("未知 EntryType 字面量：{0:?}")]
    ParseEntryType(String),
    #[error("未知 EntryStatus 字面量：{0:?}")]
    ParseEntryStatus(String),
    #[error("initial_status {0:?} 不在 promotion 图的任何节点中")]
    UnreachableInitialStatus(EntryStatus),
    #[error("schema 字段 {field} 值超出有效范围 {range}")]
    OutOfRange { field: String, range: String },
}

/// 条目类型：允许一条规则同时作用于多种类型（如 concept + entity 共享同一生命周期）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryType {
    Concept,
    Entity,
    Summary,
    Synthesis,
    Qa,
    LintReport,
    Index,
}

impl EntryType {
    /// 严格解析：未知输入显式报错，避免静默回落到 Concept 掩盖拼写错误。
    /// 同时兼容 `lint-report` 与 `lint_report` 两种分隔符。
    pub fn parse(s: &str) -> Result<Self, SchemaValidationError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "concept" => Ok(Self::Concept),
            "entity" => Ok(Self::Entity),
            "summary" => Ok(Self::Summary),
            "synthesis" => Ok(Self::Synthesis),
            "qa" => Ok(Self::Qa),
            "lint-report" | "lint_report" => Ok(Self::LintReport),
            "index" => Ok(Self::Index),
            other => Err(SchemaValidationError::ParseEntryType(other.to_string())),
        }
    }

    /// 该类型是否需要参与状态晋升流程
    pub fn participates_in_lifecycle(&self) -> bool {
        matches!(self, Self::Concept | Self::Entity)
    }

    /// 该类型创建时是否直接设为已审核
    pub fn auto_approved_on_create(&self) -> bool {
        matches!(
            self,
            Self::Summary | Self::Qa | Self::LintReport | Self::Index
        )
    }

    /// 该类型对应的正文骨架段落名称（按固定顺序）。无固定骨架的类型返回空切片。
    pub fn section_template(&self) -> &'static [&'static str] {
        match self {
            Self::Concept => &["定义", "关键要点", "本文语境", "来源引用"],
            Self::Entity => &["定义", "关键要点", "来源引用"],
            Self::Summary => &[
                "一句话摘要",
                "关键洞察",
                "提取的概念",
                "原始文章信息",
                "个人评注",
            ],
            Self::Qa => &["问题", "回答"],
            Self::Synthesis => &["研究问题", "综合分析", "关键发现", "来源列表"],
            Self::LintReport => &["检查日期", "总体健康度", "问题清单", "建议"],
            Self::Index => &[],
        }
    }
}

/// 条目状态：可晋升的 draft → in_review → approved 三态，加一个独立 needs_update。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryStatus {
    Draft,
    InReview,
    Approved,
    NeedsUpdate,
}

impl EntryStatus {
    /// 严格解析：兼容中英文标签，未知输入显式报错。
    pub fn parse(s: &str) -> Result<Self, SchemaValidationError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "草稿" | "draft" => Ok(Self::Draft),
            "审核中" | "in_review" | "inreview" => Ok(Self::InReview),
            "已审核" | "approved" => Ok(Self::Approved),
            "需更新" | "needs_update" | "needsupdate" => Ok(Self::NeedsUpdate),
            other => Err(SchemaValidationError::ParseEntryStatus(other.to_string())),
        }
    }
}

/// 单个类型的状态晋升规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleRule {
    /// 允许多个类型共享同一条规则，如 `[Concept, Entity]`。
    pub entry_types: Vec<EntryType>,
    /// 创建时的默认状态
    pub initial_status: EntryStatus,
    /// 晋升规则列表
    pub promotions: Vec<PromotionRule>,
    /// 过期天数（超过后标记为 NeedsUpdate），None 表示不过期
    pub stale_days: Option<u64>,
    /// 过期后是否自动清理（如 lint-report）
    pub auto_cleanup: bool,
}

/// 单个晋升步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionRule {
    /// 从哪个状态
    pub from_status: EntryStatus,
    /// 晋升到哪个状态
    pub to_status: EntryStatus,
    /// 晋升条件
    pub conditions: PromotionConditions,
}

/// 晋升条件。
///
/// 注意：`required_sections` 表示"晋升到目标状态所需的额外段落"，与
/// [`CompletenessConfig`] 的 `*_required_sections`（lint 基线）是**独立**的两组约束：
/// 基线无论是否晋升都要检查；晋升条件是更严格的追加要求。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionConditions {
    /// 创建后至少经过的天数
    pub min_age_days: u64,
    /// 晋升时额外要求的段落（在 lint 基线之上）
    pub required_sections: Vec<String>,
    /// 被引用次数阈值（被 summary 的 mention-page 引用）
    pub min_references: u32,
    /// 进入当前状态后无改动的天数（冷静期）
    pub cooldown_days: Option<u64>,
}

/// 标签相关配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagConfig {
    /// 种子标签（初始化时就有）
    pub seed_tags: Vec<String>,
    /// 废弃标签（禁止使用）
    pub deprecated_tags: Vec<String>,
    /// 是否允许自动扩展新标签
    pub allow_auto_extend: bool,
    /// 新标签最大名称长度（按 Unicode 字符数 `chars().count()` 计量）
    pub max_tag_name_length: u32,
    /// 每次编译最多新增标签数
    pub max_new_tags_per_ingest: u32,
    /// 标签活跃度检查窗口（天）
    pub activity_window_days: u64,
    /// 孤儿标签阈值（总使用次数低于此值且不活跃视为孤儿）
    pub orphan_threshold: u32,
    /// 休眠标签阈值（总使用次数高于此值视为休眠而非孤儿）
    pub dormant_threshold: u32,
}

/// 内容完整度检查配置（lint 基线）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletenessConfig {
    /// concept 类型必须包含的段落
    pub concept_required_sections: Vec<String>,
    /// entity 类型必须包含的段落
    pub entity_required_sections: Vec<String>,
    /// synthesis 类型必须包含的段落
    pub synthesis_required_sections: Vec<String>,
    /// 结构化引用的最小数量
    pub min_structured_references: u32,
}

/// 完整的领域 Schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainSchema {
    pub title: String,

    // --- 原有字段 ---
    pub allowed_entity_kinds: HashSet<EntityKind>,
    pub allowed_relations: HashSet<RelationKind>,
    pub min_quality_to_crystallize: f64,
    pub min_confidence_to_promote: f64,
    pub default_retention: RetentionParams,
    pub tier_half_life_days: HashMap<MemoryTier, f64>,

    // --- 新增字段 ---
    /// 条目类型生命周期规则
    #[serde(default)]
    pub lifecycle_rules: Vec<LifecycleRule>,

    /// 标签配置
    #[serde(default)]
    pub tag_config: TagConfig,

    /// 内容完整度配置
    #[serde(default)]
    pub completeness_config: CompletenessConfig,

    /// maintenance 命令的批处理大小
    #[serde(default = "default_batch_size")]
    pub maintenance_batch_size: u32,
}

fn default_batch_size() -> u32 {
    DEFAULT_MAINTENANCE_BATCH
}

impl DomainSchema {
    /// 生成一个"默认宽松"的 Schema：允许全部内置实体/关系、所有新增字段取空/默认。
    /// 主要用于单元测试与 CLI 未显式传 `--schema` 时的兜底。
    pub fn permissive_default() -> Self {
        let mut tier_half_life_days = HashMap::new();
        tier_half_life_days.insert(MemoryTier::Working, 3.0);
        tier_half_life_days.insert(MemoryTier::Episodic, 14.0);
        tier_half_life_days.insert(MemoryTier::Semantic, 60.0);
        tier_half_life_days.insert(MemoryTier::Procedural, 120.0);

        Self {
            title: "default-permissive".into(),
            allowed_entity_kinds: [
                EntityKind::Person,
                EntityKind::Project,
                EntityKind::Library,
                EntityKind::Concept,
                EntityKind::FilePath,
                EntityKind::Decision,
            ]
            .into_iter()
            .collect(),
            allowed_relations: [
                RelationKind::Uses,
                RelationKind::DependsOn,
                RelationKind::Contradicts,
                RelationKind::Caused,
                RelationKind::Fixed,
                RelationKind::Supersedes,
                RelationKind::Related,
            ]
            .into_iter()
            .collect(),
            min_quality_to_crystallize: 0.55,
            min_confidence_to_promote: 0.62,
            default_retention: RetentionParams::default(),
            tier_half_life_days,
            lifecycle_rules: Vec::new(),
            tag_config: TagConfig::default(),
            completeness_config: CompletenessConfig::default(),
            maintenance_batch_size: DEFAULT_MAINTENANCE_BATCH,
        }
    }

    pub fn relation_allowed(&self, r: &RelationKind) -> bool {
        self.allowed_relations.contains(r)
    }

    pub fn entity_kind_allowed(&self, k: &EntityKind) -> bool {
        self.allowed_entity_kinds.contains(k)
    }

    /// 从 JSON 字节反序列化，并在返回前执行语义校验。
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, SchemaLoadError> {
        let schema: Self = serde_json::from_slice(bytes)?;
        schema.validate()?;
        Ok(schema)
    }

    /// 从 UTF-8 文件读取 JSON Schema，并在返回前执行语义校验。
    pub fn from_json_path(path: &Path) -> Result<Self, SchemaLoadError> {
        let bytes = std::fs::read(path)?;
        Self::from_json_slice(&bytes)
    }

    /// 根据条目类型查找对应的生命周期规则（取首条；`validate()` 已保证唯一）。
    pub fn find_lifecycle_rule(&self, entry_type: &EntryType) -> Option<&LifecycleRule> {
        self.lifecycle_rules
            .iter()
            .find(|rule| rule.entry_types.contains(entry_type))
    }

    /// 获取标签配置
    pub fn tag_config(&self) -> &TagConfig {
        &self.tag_config
    }

    /// 按条目类型返回该类型的 lint 基线段落（concept/entity/synthesis 有各自配置，其它类型返回空）。
    pub fn required_sections_for(&self, entry_type: &EntryType) -> &[String] {
        match entry_type {
            EntryType::Concept => &self.completeness_config.concept_required_sections,
            EntryType::Entity => &self.completeness_config.entity_required_sections,
            EntryType::Synthesis => &self.completeness_config.synthesis_required_sections,
            _ => &[],
        }
    }

    /// 语义校验：
    ///
    /// 1. 任一 `EntryType` 最多出现在一条 `LifecycleRule.entry_types` 中；
    /// 2. 任一 `PromotionRule` 的 `from_status != to_status`（禁止自环）；
    /// 3. 每条规则的 promotions 形成的有向图无环（DFS 检测）；
    /// 4. `initial_status` 是 promotion 图中的可达起始节点（有出边或终端）；
    /// 5. 数值范围约束（quality/confidence ∈ (0,1]，batch_size ≥ 1，half-life > 0）。
    pub fn validate(&self) -> Result<(), SchemaValidationError> {
        // --- 规则 1：EntryType 唯一性 ---
        let mut seen: HashSet<&EntryType> = HashSet::new();
        for rule in &self.lifecycle_rules {
            for et in &rule.entry_types {
                if !seen.insert(et) {
                    return Err(SchemaValidationError::DuplicateEntryType(et.clone()));
                }
            }
        }

        // --- 规则 2 & 3：逐条 rule 检查 promotion 图 ---
        for rule in &self.lifecycle_rules {
            let mut adj: HashMap<EntryStatus, Vec<EntryStatus>> = HashMap::new();
            for p in &rule.promotions {
                if p.from_status == p.to_status {
                    return Err(SchemaValidationError::SelfLoopPromotion(p.from_status));
                }
                adj.entry(p.from_status).or_default().push(p.to_status);
            }
            if has_cycle(&adj) {
                return Err(SchemaValidationError::PromotionCycle);
            }

            // --- 规则 4：initial_status 应出现在 promotion 图中或有出边 ---
            if !rule.promotions.is_empty() {
                let mut all_nodes: HashSet<EntryStatus> = HashSet::new();
                for p in &rule.promotions {
                    all_nodes.insert(p.from_status);
                    all_nodes.insert(p.to_status);
                }
                if !all_nodes.contains(&rule.initial_status) {
                    return Err(SchemaValidationError::UnreachableInitialStatus(
                        rule.initial_status,
                    ));
                }
            }
        }

        // --- 规则 5：数值范围 ---
        if !(0.0 < self.min_quality_to_crystallize && self.min_quality_to_crystallize <= 1.0) {
            return Err(SchemaValidationError::OutOfRange {
                field: "min_quality_to_crystallize".into(),
                range: "(0, 1]".into(),
            });
        }
        if !(0.0 < self.min_confidence_to_promote && self.min_confidence_to_promote <= 1.0) {
            return Err(SchemaValidationError::OutOfRange {
                field: "min_confidence_to_promote".into(),
                range: "(0, 1]".into(),
            });
        }
        if self.maintenance_batch_size == 0 {
            return Err(SchemaValidationError::OutOfRange {
                field: "maintenance_batch_size".into(),
                range: ">= 1".into(),
            });
        }
        for (tier, hl) in &self.tier_half_life_days {
            if *hl <= 0.0 {
                return Err(SchemaValidationError::OutOfRange {
                    field: format!("tier_half_life_days.{tier:?}"),
                    range: "> 0".into(),
                });
            }
        }

        Ok(())
    }
}

/// 基于 DFS 的有向图环检测。节点集合由 `adj` 的键隐式定义（未出现的状态视为终态）。
fn has_cycle(adj: &HashMap<EntryStatus, Vec<EntryStatus>>) -> bool {
    /// DFS 三色标记：White=未访问, Gray=访问中, Black=已完成。
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Color {
        White,
        Gray,
        Black,
    }

    let mut color: HashMap<EntryStatus, Color> = adj.keys().map(|k| (*k, Color::White)).collect();

    fn dfs(
        node: EntryStatus,
        adj: &HashMap<EntryStatus, Vec<EntryStatus>>,
        color: &mut HashMap<EntryStatus, Color>,
    ) -> bool {
        color.insert(node, Color::Gray);
        if let Some(nexts) = adj.get(&node) {
            for n in nexts {
                match color.get(n).copied().unwrap_or(Color::White) {
                    Color::Gray => return true,
                    Color::White => {
                        if dfs(*n, adj, color) {
                            return true;
                        }
                    }
                    Color::Black => {}
                }
            }
        }
        color.insert(node, Color::Black);
        false
    }

    let nodes: Vec<EntryStatus> = color.keys().copied().collect();
    for n in nodes {
        if color.get(&n).copied() == Some(Color::White) && dfs(n, adj, &mut color) {
            return true;
        }
    }
    false
}

impl Default for TagConfig {
    /// core 层默认**不**夹带任何业务标签；种子标签应在用户的 `DomainSchema.json` 中声明。
    fn default() -> Self {
        Self {
            seed_tags: Vec::new(),
            deprecated_tags: Vec::new(),
            allow_auto_extend: true,
            max_tag_name_length: 10,
            max_new_tags_per_ingest: 1,
            activity_window_days: 30,
            orphan_threshold: 5,
            dormant_threshold: 10,
        }
    }
}

impl Default for CompletenessConfig {
    fn default() -> Self {
        Self {
            concept_required_sections: vec!["定义".into(), "关键要点".into(), "来源引用".into()],
            entity_required_sections: vec!["简介".into(), "关键数据".into(), "来源引用".into()],
            synthesis_required_sections: vec![
                "研究问题".into(),
                "综合分析".into(),
                "关键发现".into(),
                "来源列表".into(),
            ],
            min_structured_references: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rule(
        types: Vec<EntryType>,
        promotions: Vec<(EntryStatus, EntryStatus)>,
    ) -> LifecycleRule {
        LifecycleRule {
            entry_types: types,
            initial_status: EntryStatus::Draft,
            promotions: promotions
                .into_iter()
                .map(|(a, b)| PromotionRule {
                    from_status: a,
                    to_status: b,
                    conditions: PromotionConditions {
                        min_age_days: 0,
                        required_sections: Vec::new(),
                        min_references: 0,
                        cooldown_days: None,
                    },
                })
                .collect(),
            stale_days: None,
            auto_cleanup: false,
        }
    }

    #[test]
    fn parse_entry_type_rejects_unknown() {
        assert!(matches!(
            EntryType::parse("zzz"),
            Err(SchemaValidationError::ParseEntryType(_))
        ));
        assert!(matches!(
            EntryType::parse("lint-report"),
            Ok(EntryType::LintReport)
        ));
    }

    #[test]
    fn parse_entry_status_rejects_unknown() {
        assert!(matches!(
            EntryStatus::parse("??"),
            Err(SchemaValidationError::ParseEntryStatus(_))
        ));
        assert_eq!(EntryStatus::parse("草稿").unwrap(), EntryStatus::Draft);
    }

    #[test]
    fn validate_detects_duplicate_entry_type() {
        let mut schema = DomainSchema::permissive_default();
        schema.lifecycle_rules = vec![
            make_rule(vec![EntryType::Concept], vec![]),
            make_rule(vec![EntryType::Concept], vec![]),
        ];
        assert!(matches!(
            schema.validate(),
            Err(SchemaValidationError::DuplicateEntryType(
                EntryType::Concept
            ))
        ));
    }

    #[test]
    fn validate_detects_self_loop() {
        let mut schema = DomainSchema::permissive_default();
        schema.lifecycle_rules = vec![make_rule(
            vec![EntryType::Concept],
            vec![(EntryStatus::Draft, EntryStatus::Draft)],
        )];
        assert!(matches!(
            schema.validate(),
            Err(SchemaValidationError::SelfLoopPromotion(EntryStatus::Draft))
        ));
    }

    #[test]
    fn validate_detects_cycle() {
        let mut schema = DomainSchema::permissive_default();
        schema.lifecycle_rules = vec![make_rule(
            vec![EntryType::Concept],
            vec![
                (EntryStatus::Draft, EntryStatus::InReview),
                (EntryStatus::InReview, EntryStatus::Approved),
                (EntryStatus::Approved, EntryStatus::Draft),
            ],
        )];
        assert_eq!(
            schema.validate(),
            Err(SchemaValidationError::PromotionCycle)
        );
    }

    #[test]
    fn validate_accepts_dag() {
        let mut schema = DomainSchema::permissive_default();
        schema.lifecycle_rules = vec![make_rule(
            vec![EntryType::Concept, EntryType::Entity],
            vec![
                (EntryStatus::Draft, EntryStatus::InReview),
                (EntryStatus::InReview, EntryStatus::Approved),
            ],
        )];
        assert!(schema.validate().is_ok());
    }

    #[test]
    fn validate_rejects_unreachable_initial_status() {
        let mut schema = DomainSchema::permissive_default();
        let mut rule = make_rule(
            vec![EntryType::Concept],
            vec![(EntryStatus::Draft, EntryStatus::Approved)],
        );
        rule.initial_status = EntryStatus::NeedsUpdate;
        schema.lifecycle_rules = vec![rule];
        assert!(matches!(
            schema.validate(),
            Err(SchemaValidationError::UnreachableInitialStatus(
                EntryStatus::NeedsUpdate
            ))
        ));
    }

    #[test]
    fn validate_rejects_out_of_range_quality() {
        let mut schema = DomainSchema::permissive_default();
        schema.min_quality_to_crystallize = 1.5;
        assert!(matches!(
            schema.validate(),
            Err(SchemaValidationError::OutOfRange { field, .. }) if field == "min_quality_to_crystallize"
        ));
    }

    #[test]
    fn validate_rejects_zero_batch_size() {
        let mut schema = DomainSchema::permissive_default();
        schema.maintenance_batch_size = 0;
        assert!(matches!(
            schema.validate(),
            Err(SchemaValidationError::OutOfRange { field, .. }) if field == "maintenance_batch_size"
        ));
    }

    #[test]
    fn validate_rejects_negative_half_life() {
        let mut schema = DomainSchema::permissive_default();
        schema.tier_half_life_days.insert(MemoryTier::Working, -1.0);
        assert!(matches!(
            schema.validate(),
            Err(SchemaValidationError::OutOfRange { .. })
        ));
    }

    #[test]
    fn default_tag_config_has_no_business_seed_tags() {
        let cfg = TagConfig::default();
        assert!(cfg.seed_tags.is_empty(), "core 层默认不应夹带业务标签");
        assert!(cfg.deprecated_tags.is_empty());
    }

    #[test]
    fn required_sections_for_falls_back_to_empty() {
        let schema = DomainSchema::permissive_default();
        assert!(!schema.required_sections_for(&EntryType::Concept).is_empty());
        assert!(schema.required_sections_for(&EntryType::Qa).is_empty());
        assert!(schema.required_sections_for(&EntryType::Index).is_empty());
    }
}
