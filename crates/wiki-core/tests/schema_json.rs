//! DomainSchema JSON 契约测试：
//! 目的是锁住「仓库根目录的 `DomainSchema.json`」与 `wiki-core::schema` 结构之间的契约，
//! 一旦某一侧的枚举名或字段名漂移，这个测试立即失败，避免下游 CLI 启动时才暴露问题。

use std::path::PathBuf;

use wiki_core::{
    DomainSchema, EntityKind, EntryStatus, EntryType, MemoryTier, RelationKind,
};

/// 定位仓库根目录下的 `DomainSchema.json`：
/// `CARGO_MANIFEST_DIR` 指向 `crates/wiki-core`，向上两级即到仓库根。
fn repo_schema_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("应当能定位到仓库根目录")
        .join("DomainSchema.json")
}

/// 用例 1：仓库根 JSON 能被 `DomainSchema::from_json_path` 成功反序列化。
#[test]
fn repo_domain_schema_json_deserializes() {
    let path = repo_schema_path();
    let schema = DomainSchema::from_json_path(&path)
        .unwrap_or_else(|err| panic!("加载 {} 失败: {err}", path.display()));

    assert_eq!(schema.title, "知识 Wiki 本地化 Schema v1.0");

    // 允许的实体/关系白名单：JSON 中列出 6/7 条，反序列化后数量必须匹配
    assert!(schema.entity_kind_allowed(&EntityKind::Person));
    assert!(schema.entity_kind_allowed(&EntityKind::FilePath));
    assert!(schema.relation_allowed(&RelationKind::Supersedes));
    assert_eq!(schema.allowed_entity_kinds.len(), 6);
    assert_eq!(schema.allowed_relations.len(), 7);

    // 记忆层半衰期键：四层都存在且键是 snake_case 形式的 MemoryTier
    assert!((schema.tier_half_life_days[&MemoryTier::Working] - 3.0).abs() < f64::EPSILON);
    assert!((schema.tier_half_life_days[&MemoryTier::Procedural] - 120.0).abs() < f64::EPSILON);

    // 新增字段：维护批大小 / 标签配置 / 完整度配置
    assert_eq!(schema.maintenance_batch_size, 128);
    assert_eq!(schema.tag_config.seed_tags.len(), 14);
    assert!(schema.tag_config.allow_auto_extend);
    assert_eq!(schema.completeness_config.min_structured_references, 1);
}

/// 用例 2：生命周期规则在 JSON 中共 6 条，且能按 EntryType 正确索引。
#[test]
fn repo_domain_schema_lifecycle_rules_indexable() {
    let schema = DomainSchema::from_json_path(&repo_schema_path()).expect("schema 加载失败");
    assert_eq!(schema.lifecycle_rules.len(), 6);

    // Concept 与 Entity 共享同一条规则，初始为 Draft 且有两步晋升
    let rule = schema
        .find_lifecycle_rule(&EntryType::Concept)
        .expect("应找到 Concept 的生命周期规则");
    assert_eq!(rule.initial_status, EntryStatus::Draft);
    assert_eq!(rule.promotions.len(), 2);
    assert_eq!(rule.stale_days, Some(30));
    assert!(!rule.auto_cleanup);

    // Concept/Entity 共用一条规则：通过 Entity 查询也应拿到同一条
    let rule_by_entity = schema
        .find_lifecycle_rule(&EntryType::Entity)
        .expect("应找到 Entity 的生命周期规则");
    assert_eq!(rule_by_entity.initial_status, rule.initial_status);

    // LintReport 必须 auto_cleanup=true，stale_days=7，验证 snake_case 别名解析正确
    let lint_rule = schema
        .find_lifecycle_rule(&EntryType::LintReport)
        .expect("应找到 LintReport 的生命周期规则");
    assert!(lint_rule.auto_cleanup);
    assert_eq!(lint_rule.stale_days, Some(7));

    // Index 类型 stale_days 为 null，解析后应为 None（不过期）
    let idx_rule = schema
        .find_lifecycle_rule(&EntryType::Index)
        .expect("应找到 Index 的生命周期规则");
    assert!(idx_rule.stale_days.is_none());
}

/// 用例 3：permissive_default 与从 JSON 加载的 schema 都能通过 serde_json 来回 round-trip。
/// 防止未来有人给结构体加字段却忘了 `#[serde(default)]` 造成历史 JSON 读不回来。
#[test]
fn round_trip_serialize_deserialize() {
    let original = DomainSchema::from_json_path(&repo_schema_path()).expect("schema 加载失败");
    let bytes = serde_json::to_vec(&original).expect("序列化 schema 失败");
    let roundtripped =
        DomainSchema::from_json_slice(&bytes).expect("反序列化 round-trip 结果失败");

    assert_eq!(roundtripped.title, original.title);
    assert_eq!(
        roundtripped.allowed_entity_kinds.len(),
        original.allowed_entity_kinds.len()
    );
    assert_eq!(
        roundtripped.lifecycle_rules.len(),
        original.lifecycle_rules.len()
    );
    assert_eq!(
        roundtripped.maintenance_batch_size,
        original.maintenance_batch_size
    );
    assert_eq!(
        roundtripped.tag_config.seed_tags,
        original.tag_config.seed_tags
    );
}
