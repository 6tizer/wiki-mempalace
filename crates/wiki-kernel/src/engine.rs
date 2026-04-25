use time::OffsetDateTime;

use crate::hooks::{NoopWikiHook, WikiHook};
use crate::memory::InMemoryStore;
use std::path::Path;
use wiki_storage::{StorageError, WikiRepository};

use crate::search_ports::SearchPorts;
use wiki_core::{
    advance_tier, apply_time_decay_to_confidence, document_visible_to_viewer, draft_from_session,
    extract_wikilinks, merge_sources_confidence, normalize_and_validate_tags,
    reciprocal_rank_fusion, redact_for_ingest, reinforce_claim, retention_strength,
    supersede_claim, walk_entities, AuditOperation, AuditRecord, Claim, ClaimId, ContradictionHint,
    CrystallizationDraft, DomainSchema, Entity, EntityId, EntryStatus, EntryType, GapFinding,
    GraphWalkOptions, LintFinding, LintSeverity, MemoryTier, PageId, QueryContext, RankedDoc,
    RawArtifact, RelationKind, SchemaLoadError, Scope, SessionCrystallizationInput, SourceId,
    TagPolicyError, TypedEdge, WikiEvent,
};

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    Schema(#[from] SchemaLoadError),
    #[error("claim not found: {0:?}")]
    ClaimNotFound(ClaimId),
    #[error("schema rejected relation: {0:?}")]
    RelationNotAllowed(RelationKind),
    #[error("schema rejected entity kind")]
    EntityKindNotAllowed,
    #[error("promotion threshold not met")]
    PromotionDenied,
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("scope denied: resource not visible to this viewer")]
    ScopeDenied,
    #[error(transparent)]
    TagPolicy(#[from] TagPolicyError),
}

/// 页面状态晋升失败的细粒度错误
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum PromotePageError {
    #[error("页面不存在")]
    UnknownPage,
    #[error("页面缺少 entry_type，无法匹配生命周期规则")]
    NoEntryType,
    #[error("该 entry_type 没有对应的 lifecycle rule")]
    NoRule,
    #[error("没有匹配的晋升路径 from={from:?} to={to:?}")]
    NoPromotion { from: EntryStatus, to: EntryStatus },
    #[error("创建天数不足：需要 {need} 天，当前 {have} 天")]
    AgeTooYoung { need: u64, have: u64 },
    #[error("缺少必需段落：{0:?}")]
    MissingSections(Vec<String>),
    #[error("引用次数不足：需要 {need}，当前 {have}")]
    NotEnoughReferences { need: u32, have: u32 },
    #[error("冷却期未满：需要 {need} 天，已等待 {have} 天")]
    Cooldown { need: u64, have: u64 },
}

/// 编排 ingest / 写入 / 取代 / 结晶 / lint / 混合排序 的内存参考实现。
pub struct LlmWikiEngine<H: WikiHook = NoopWikiHook> {
    pub schema: DomainSchema,
    pub store: InMemoryStore,
    pub audits: Vec<AuditRecord>,
    pub outbox: Vec<WikiEvent>,
    hooks: H,
}

impl LlmWikiEngine<NoopWikiHook> {
    pub fn new(schema: DomainSchema) -> Self {
        Self::with_hooks(schema, NoopWikiHook)
    }

    /// 从 JSON 文件加载 [`DomainSchema`] 并创建引擎。
    pub fn from_schema_json_path(path: &Path) -> Result<Self, EngineError> {
        let schema = DomainSchema::from_json_path(path)?;
        Ok(Self::new(schema))
    }
}

impl<H: WikiHook> LlmWikiEngine<H> {
    pub fn with_hooks(schema: DomainSchema, hooks: H) -> Self {
        Self {
            schema,
            store: InMemoryStore::default(),
            audits: Vec::new(),
            outbox: Vec::new(),
            hooks,
        }
    }

    fn emit(&mut self, e: WikiEvent) {
        self.outbox.push(e.clone());
        self.hooks.on_event(&e);
    }

    fn audit(&mut self, op: AuditOperation, actor: &str, summary: impl Into<String>) {
        self.audits
            .push(AuditRecord::new(op, actor.to_string(), summary));
    }

    /// 原始层 ingest：先脱敏，再入库，写审计与事件。
    pub fn ingest_raw(
        &mut self,
        uri: impl Into<String>,
        body: &str,
        scope: Scope,
        actor: &str,
    ) -> SourceId {
        self.ingest_raw_validated(uri, body, scope, actor, Vec::new())
    }

    pub fn ingest_raw_with_tags<I, S>(
        &mut self,
        uri: impl Into<String>,
        body: &str,
        scope: Scope,
        actor: &str,
        tags: I,
    ) -> Result<SourceId, EngineError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let tags = normalize_and_validate_tags(tags, &self.schema)?;
        Ok(self.ingest_raw_validated(uri, body, scope, actor, tags))
    }

    fn ingest_raw_validated(
        &mut self,
        uri: impl Into<String>,
        body: &str,
        scope: Scope,
        actor: &str,
        tags: Vec<String>,
    ) -> SourceId {
        let (clean, findings) = redact_for_ingest(body);
        let mut art = RawArtifact::new(uri, clean, scope);
        art.tags = tags;
        let id = art.id;
        self.store.sources.insert(id, art);
        self.audit(
            AuditOperation::IngestSource,
            actor,
            format!("ingested {} redactions={}", id.0, findings.len()),
        );
        self.emit(WikiEvent::SourceIngested {
            source_id: id,
            redacted: !findings.is_empty(),
            at: OffsetDateTime::now_utc(),
        });
        if !findings.is_empty() {
            self.audit(
                AuditOperation::RedactSensitive,
                actor,
                format!("redaction findings={}", findings.len()),
            );
        }
        id
    }

    pub fn file_claim(
        &mut self,
        text: impl Into<String>,
        scope: Scope,
        tier: MemoryTier,
        actor: &str,
    ) -> ClaimId {
        self.file_claim_validated(text, scope, tier, actor, Vec::new())
    }

    pub fn file_claim_with_tags<I, S>(
        &mut self,
        text: impl Into<String>,
        scope: Scope,
        tier: MemoryTier,
        actor: &str,
        tags: I,
    ) -> Result<ClaimId, EngineError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let tags = normalize_and_validate_tags(tags, &self.schema)?;
        Ok(self.file_claim_validated(text, scope, tier, actor, tags))
    }

    fn file_claim_validated(
        &mut self,
        text: impl Into<String>,
        scope: Scope,
        tier: MemoryTier,
        actor: &str,
        tags: Vec<String>,
    ) -> ClaimId {
        let mut c = Claim::new(text, scope, tier);
        c.tags = tags;
        let id = c.id;
        self.store.claims.insert(id, c);
        self.audit(AuditOperation::WriteClaim, actor, format!("claim {}", id.0));
        self.emit(WikiEvent::ClaimUpserted {
            claim_id: id,
            at: OffsetDateTime::now_utc(),
        });
        id
    }

    pub fn attach_sources(
        &mut self,
        claim_id: ClaimId,
        sources: &[SourceId],
    ) -> Result<(), EngineError> {
        let claim = self
            .store
            .claims
            .get_mut(&claim_id)
            .ok_or(EngineError::ClaimNotFound(claim_id))?;
        for s in sources {
            if !claim.source_ids.contains(s) {
                claim.source_ids.push(*s);
            }
        }
        merge_sources_confidence(claim, sources.len());
        reinforce_claim(claim, OffsetDateTime::now_utc(), 0.03);
        Ok(())
    }

    pub fn supersede(
        &mut self,
        old_id: ClaimId,
        new_text: impl Into<String>,
        scope: Scope,
        tier: MemoryTier,
        actor: &str,
    ) -> Result<ClaimId, EngineError> {
        let old = self
            .store
            .claims
            .get_mut(&old_id)
            .ok_or(EngineError::ClaimNotFound(old_id))?
            .clone();
        let mut new_c = Claim::new(new_text, scope, tier);
        let now = OffsetDateTime::now_utc();
        if let Some(old_mut) = self.store.claims.get_mut(&old_id) {
            supersede_claim(old_mut, &mut new_c, now);
        }
        let new_id = new_c.id;
        self.store.claims.insert(new_id, new_c);
        self.audit(
            AuditOperation::SupersedeClaim,
            actor,
            format!("{} supersedes {}", new_id.0, old.id.0),
        );
        self.emit(WikiEvent::ClaimSuperseded {
            old: old_id,
            new: new_id,
            at: now,
        });
        Ok(new_id)
    }

    pub fn add_entity(&mut self, entity: Entity) -> Result<(), EngineError> {
        if !self.schema.entity_kind_allowed(&entity.kind) {
            return Err(EngineError::EntityKindNotAllowed);
        }
        self.store.entities.insert(entity.id, entity);
        Ok(())
    }

    pub fn add_edge(&mut self, edge: TypedEdge) -> Result<(), EngineError> {
        if !self.schema.relation_allowed(&edge.relation) {
            return Err(EngineError::RelationNotAllowed(edge.relation.clone()));
        }
        self.store.edges.push(edge);
        Ok(())
    }

    /// 若质量与置信度达到 Schema 阈值，则沿巩固阶梯晋升一级。
    pub fn promote_if_qualified(
        &mut self,
        claim_id: ClaimId,
        actor: &str,
        viewer: &Scope,
    ) -> Result<(), EngineError> {
        let claim = self
            .store
            .claims
            .get(&claim_id)
            .ok_or(EngineError::ClaimNotFound(claim_id))?;
        if !document_visible_to_viewer(&claim.scope, viewer) {
            return Err(EngineError::ScopeDenied);
        }
        if claim.confidence < self.schema.min_confidence_to_promote
            || claim.quality_score < self.schema.min_quality_to_crystallize
        {
            return Err(EngineError::PromotionDenied);
        }
        let claim = self.store.claims.get_mut(&claim_id).unwrap();
        advance_tier(claim);
        self.audit(
            AuditOperation::WriteClaim,
            actor,
            format!("promoted claim {}", claim_id.0),
        );
        Ok(())
    }

    /// 页面生命周期状态晋升，按 schema 的 PromotionConditions 逐项检查。
    /// `force` 为 true 时跳过全部条件检查，直接变更状态。
    pub fn promote_page(
        &mut self,
        page_id: PageId,
        to_status: EntryStatus,
        actor: &str,
        now: OffsetDateTime,
        force: bool,
    ) -> Result<(), PromotePageError> {
        use wiki_core::extract_headings;

        let page = self
            .store
            .pages
            .get(&page_id)
            .ok_or(PromotePageError::UnknownPage)?;

        let entry_type = page
            .entry_type
            .clone()
            .ok_or(PromotePageError::NoEntryType)?;

        let rule = self
            .schema
            .find_lifecycle_rule(&entry_type)
            .ok_or(PromotePageError::NoRule)?;

        let from_status = page.status;

        // 查找匹配的 promotion rule
        let promo = rule
            .promotions
            .iter()
            .find(|p| p.from_status == from_status && p.to_status == to_status)
            .ok_or(PromotePageError::NoPromotion {
                from: from_status,
                to: to_status,
            })?;

        if !force {
            let conditions = &promo.conditions;

            // min_age_days：从 page.created_at 起算（历史 JSON 回落到 updated_at）
            let age_secs = (now - page.age_from()).whole_seconds();
            let age_days = age_secs as u64 / 86400;
            if age_days < conditions.min_age_days {
                return Err(PromotePageError::AgeTooYoung {
                    need: conditions.min_age_days,
                    have: age_days,
                });
            }

            // required_sections：检查 page markdown 中的 heading
            let headings = extract_headings(&page.markdown);
            let missing: Vec<String> = conditions
                .required_sections
                .iter()
                .filter(|s| !headings.iter().any(|h| h == *s))
                .cloned()
                .collect();
            if !missing.is_empty() {
                return Err(PromotePageError::MissingSections(missing));
            }

            // min_references：统计其它 page 的 outbound_page_titles 中包含本 page title 的数量
            let title = page.title.clone();
            let ref_count = self
                .store
                .pages
                .values()
                .filter(|p| p.id != page_id)
                .filter(|p| p.outbound_page_titles.contains(&title))
                .count() as u32;
            if ref_count < conditions.min_references {
                return Err(PromotePageError::NotEnoughReferences {
                    need: conditions.min_references,
                    have: ref_count,
                });
            }

            // cooldown_days：从进入当前 status 起算，内容编辑不重置
            if let Some(cd) = conditions.cooldown_days {
                let cooldown_secs = (now - page.status_since()).whole_seconds();
                let cooldown_days = cooldown_secs as u64 / 86400;
                if cooldown_days < cd {
                    return Err(PromotePageError::Cooldown {
                        need: cd,
                        have: cooldown_days,
                    });
                }
            }
        }

        // 通过检查 → 写入
        let page = self.store.pages.get_mut(&page_id).unwrap();
        page.status = to_status;
        page.updated_at = now;
        page.status_entered_at = Some(now);

        self.emit(WikiEvent::PageStatusChanged {
            page_id,
            from: from_status,
            to: to_status,
            actor: actor.to_string(),
            at: now,
        });
        Ok(())
    }

    /// 标记过期页面为 NeedsUpdate（auto_cleanup == false 的 rule）。
    /// 返回被标记的页面数量。
    pub fn mark_stale_pages(&mut self, now: OffsetDateTime) -> u32 {
        let mut count = 0u32;
        // 收集需要检查的 (entry_type_set, stale_days)
        let mut rules_to_check: Vec<(std::collections::HashSet<EntryType>, u64)> = Vec::new();
        for rule in &self.schema.lifecycle_rules {
            if let Some(d) = rule.stale_days {
                if !rule.auto_cleanup {
                    let types: std::collections::HashSet<EntryType> =
                        rule.entry_types.iter().cloned().collect();
                    rules_to_check.push((types, d));
                }
            }
        }
        if rules_to_check.is_empty() {
            return 0;
        }
        // 收集需要变更的 page_id（避免借用冲突）
        let page_ids: Vec<PageId> = self.store.pages.keys().copied().collect();
        for pid in page_ids {
            let page = self.store.pages.get(&pid).unwrap();
            let et = match page.entry_type {
                Some(ref et) => et.clone(),
                None => continue,
            };
            if page.status == EntryStatus::NeedsUpdate {
                continue;
            }
            let should_mark = rules_to_check.iter().any(|(types, d)| {
                types.contains(&et) && {
                    let age_days = (now - page.updated_at).whole_seconds() as u64 / 86400;
                    age_days > *d
                }
            });
            if should_mark {
                let page = self.store.pages.get_mut(&pid).unwrap();
                let from = page.status;
                page.status = EntryStatus::NeedsUpdate;
                page.status_entered_at = Some(now);
                self.emit(WikiEvent::PageStatusChanged {
                    page_id: pid,
                    from,
                    to: EntryStatus::NeedsUpdate,
                    actor: "maintenance".to_string(),
                    at: now,
                });
                count += 1;
            }
        }
        count
    }

    /// 清理过期页面（auto_cleanup == true 的 rule）。
    /// 返回被删除的页面数量。
    pub fn cleanup_expired_pages(&mut self, now: OffsetDateTime) -> u32 {
        let mut count = 0u32;
        let mut rules_to_check: Vec<(std::collections::HashSet<EntryType>, u64)> = Vec::new();
        for rule in &self.schema.lifecycle_rules {
            if let Some(d) = rule.stale_days {
                if rule.auto_cleanup {
                    let types: std::collections::HashSet<EntryType> =
                        rule.entry_types.iter().cloned().collect();
                    rules_to_check.push((types, d));
                }
            }
        }
        if rules_to_check.is_empty() {
            return 0;
        }
        let page_ids: Vec<PageId> = self.store.pages.keys().copied().collect();
        let mut to_remove: Vec<PageId> = Vec::new();
        for pid in page_ids {
            let page = self.store.pages.get(&pid).unwrap();
            let et = match page.entry_type {
                Some(ref et) => et.clone(),
                None => continue,
            };
            let should_remove = rules_to_check.iter().any(|(types, d)| {
                types.contains(&et) && {
                    let age_days = (now - page.updated_at).whole_seconds() as u64 / 86400;
                    age_days > *d
                }
            });
            if should_remove {
                to_remove.push(pid);
            }
        }
        for pid in to_remove {
            self.store.pages.remove(&pid);
            self.emit(WikiEvent::PageDeleted {
                page_id: pid,
                at: now,
            });
            count += 1;
        }
        count
    }

    pub fn set_claim_quality(&mut self, claim_id: ClaimId, q: f64) -> Result<(), EngineError> {
        let c = self
            .store
            .claims
            .get_mut(&claim_id)
            .ok_or(EngineError::ClaimNotFound(claim_id))?;
        c.quality_score = q.clamp(0.0, 1.0);
        Ok(())
    }

    /// 结晶：生成 `WikiPage` 草稿与候选断言文本。
    pub fn crystallize(
        &mut self,
        input: SessionCrystallizationInput,
        actor: &str,
    ) -> Result<CrystallizationDraft, EngineError> {
        let draft = draft_from_session(input);
        let page_id = draft.page.id;
        self.store.pages.insert(page_id, draft.page.clone());
        self.audit(
            AuditOperation::CrystallizeSession,
            actor,
            format!("crystallized page {}", page_id.0),
        );
        self.emit(WikiEvent::SessionCrystallized {
            page_id,
            at: OffsetDateTime::now_utc(),
        });
        Ok(draft)
    }

    pub fn hybrid_rrf(
        &self,
        bm25_ids: Vec<String>,
        vector_ids: Vec<String>,
        graph_ids: Vec<String>,
        k: f64,
    ) -> Vec<RankedDoc> {
        reciprocal_rank_fusion(&[bm25_ids, vector_ids, graph_ids], k)
    }

    /// 内置内存三路 stub → RRF → 保留强度加权（不写审计）。
    pub fn query_ranked_memory(
        &self,
        ctx: &QueryContext<'_>,
        now: OffsetDateTime,
        vector_rank_override: Option<Vec<String>>,
        graph_rank_override: Option<Vec<String>>,
    ) -> Vec<(String, f64)> {
        let ports = crate::InMemorySearchPorts::new(&self.store, ctx.viewer_scope.clone());
        self.query_ranked_with_ports(ctx, now, &ports, vector_rank_override, graph_rank_override)
    }

    /// 使用自定义 SearchPorts 做三路召回。
    pub fn query_ranked_with_ports(
        &self,
        ctx: &QueryContext<'_>,
        now: OffsetDateTime,
        ports: &dyn SearchPorts,
        vector_rank_override: Option<Vec<String>>,
        graph_rank_override: Option<Vec<String>>,
    ) -> Vec<(String, f64)> {
        query_ranked_with_ports(
            &self.schema,
            &self.store,
            ctx,
            ports,
            now,
            vector_rank_override,
            graph_rank_override,
        )
    }

    /// 三路端口召回 → RRF →（对 `claim:`）保留强度加权；并写审计与 `QueryServed` 事件。
    pub fn query_pipeline_memory(
        &mut self,
        ctx: &QueryContext<'_>,
        now: OffsetDateTime,
        actor: &str,
        vector_rank_override: Option<Vec<String>>,
        graph_rank_override: Option<Vec<String>>,
    ) -> Vec<(String, f64)> {
        let ranked = self.query_ranked_memory(ctx, now, vector_rank_override, graph_rank_override);
        let top: Vec<String> = ranked.iter().take(24).map(|(id, _)| id.clone()).collect();
        self.record_query(ctx.query, top, actor);
        ranked
    }

    /// 使用自定义 SearchPorts 的完整 query pipeline。
    pub fn query_pipeline_with_ports(
        &mut self,
        ctx: &QueryContext<'_>,
        now: OffsetDateTime,
        actor: &str,
        ports: &dyn SearchPorts,
        vector_rank_override: Option<Vec<String>>,
        graph_rank_override: Option<Vec<String>>,
    ) -> Vec<(String, f64)> {
        let ranked = self.query_ranked_with_ports(
            ctx,
            now,
            ports,
            vector_rank_override,
            graph_rank_override,
        );
        let top: Vec<String> = ranked.iter().take(24).map(|(id, _)| id.clone()).collect();
        self.record_query(ctx.query, top, actor);
        ranked
    }

    pub fn expand_graph(&self, seeds: &[EntityId], opts: &GraphWalkOptions) -> Vec<EntityId> {
        let snap = self.store.graph_snapshot();
        walk_entities(&snap, seeds, opts)
    }

    /// 结合 RRF 与保留强度：仅对 `claim:` doc 二次加权；`page:` / `entity:` 乘子为 1。
    pub fn rank_claims_with_retention(
        &self,
        fused: &[RankedDoc],
        now: OffsetDateTime,
    ) -> Vec<(String, f64)> {
        self.rank_docs_with_retention(fused, now)
    }

    fn rank_docs_with_retention(
        &self,
        fused: &[RankedDoc],
        now: OffsetDateTime,
    ) -> Vec<(String, f64)> {
        rank_fused_with_retention(&self.schema, &self.store, fused, now)
    }

    pub fn run_basic_lint(
        &mut self,
        actor: &str,
        viewer_scope: Option<&Scope>,
    ) -> Vec<LintFinding> {
        let visible = |s: &Scope| match viewer_scope {
            None => true,
            Some(v) => document_visible_to_viewer(s, v),
        };
        for p in self.store.pages.values_mut() {
            if !visible(&p.scope) {
                continue;
            }
            p.refresh_outbound_links();
        }
        let findings = collect_basic_lint_findings(&self.schema, &self.store, viewer_scope);
        self.audit(
            AuditOperation::RunLint,
            actor,
            format!("lint findings={}", findings.len()),
        );
        self.emit(WikiEvent::LintRunFinished {
            findings: findings.len(),
            at: OffsetDateTime::now_utc(),
        });
        findings
    }

    pub fn naive_contradiction_pairs(
        &self,
        viewer_scope: Option<&Scope>,
    ) -> Vec<ContradictionHint> {
        let mut hints = Vec::new();
        let visible = |s: &Scope| match viewer_scope {
            None => true,
            Some(v) => document_visible_to_viewer(s, v),
        };
        let ids: Vec<ClaimId> = self
            .store
            .claims
            .iter()
            .filter(|(_, c)| visible(&c.scope))
            .map(|(id, _)| *id)
            .collect();
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let a = &self.store.claims[&ids[i]];
                let b = &self.store.claims[&ids[j]];
                if a.stale || b.stale {
                    continue;
                }
                if contradicts_heuristic(&a.text, &b.text) {
                    hints.push(ContradictionHint {
                        a: a.id,
                        b: b.id,
                        reason: "heuristic negation / mismatch".into(),
                    });
                }
            }
        }
        hints
    }

    /// 批处理：对所有断言按统一半衰期衰减置信度（遗忘曲线的时间分量）。
    pub fn apply_confidence_decay_all(&mut self, now: OffsetDateTime, half_life_days: f64) {
        for c in self.store.claims.values_mut() {
            apply_time_decay_to_confidence(c, now, half_life_days);
        }
    }

    pub fn record_query(
        &mut self,
        query_fingerprint: impl Into<String>,
        top_doc_ids: Vec<String>,
        actor: &str,
    ) {
        let fp = query_fingerprint.into();
        self.audit(AuditOperation::RunQuery, actor, format!("query fp={fp}"));
        self.emit(WikiEvent::QueryServed {
            query_fingerprint: fp,
            top_doc_ids,
            at: OffsetDateTime::now_utc(),
        });
    }

    pub fn load_from_repo<R: WikiRepository>(
        schema: DomainSchema,
        repo: &R,
        hooks: H,
    ) -> Result<Self, EngineError> {
        let snap = repo.load_snapshot()?;
        Ok(Self {
            schema,
            store: InMemoryStore::from_snapshot(snap.clone()),
            audits: snap.audits,
            outbox: Vec::new(),
            hooks,
        })
    }

    pub fn save_to_repo<R: WikiRepository>(&self, repo: &R) -> Result<(), EngineError> {
        let snap = self.store.to_snapshot(&self.audits);
        repo.save_snapshot(&snap)?;
        Ok(())
    }

    pub fn save_to_repo_and_flush_outbox<R: WikiRepository>(
        &mut self,
        repo: &R,
    ) -> Result<usize, EngineError> {
        self.save_to_repo_and_flush_outbox_with_policy(repo, 128, 3)
    }

    pub fn save_to_repo_and_flush_outbox_with_policy<R: WikiRepository>(
        &mut self,
        repo: &R,
        _batch_size: usize,
        retry_count: usize,
    ) -> Result<usize, EngineError> {
        let snap = self.store.to_snapshot(&self.audits);
        let mut last_err: Option<EngineError> = None;
        for _ in 0..=retry_count {
            match repo.save_snapshot_and_append_outbox(&snap, &self.outbox) {
                Ok(n) => {
                    self.outbox.clear();
                    return Ok(n);
                }
                Err(err) => {
                    last_err = Some(err.into());
                }
            }
        }
        Err(last_err.expect("retry loop runs at least once"))
    }

    pub fn flush_outbox_to_repo<R: WikiRepository>(
        &mut self,
        repo: &R,
    ) -> Result<usize, EngineError> {
        self.flush_outbox_to_repo_with_policy(repo, 128, 3)
    }

    pub fn flush_outbox_to_repo_with_policy<R: WikiRepository>(
        &mut self,
        repo: &R,
        batch_size: usize,
        retry_count: usize,
    ) -> Result<usize, EngineError> {
        let mut n = 0usize;
        let size = batch_size.max(1);
        while n < self.outbox.len() {
            let end = usize::min(n + size, self.outbox.len());
            for event in &self.outbox[n..end] {
                let mut last_err: Option<EngineError> = None;
                for _ in 0..=retry_count {
                    match repo.append_outbox(event) {
                        Ok(()) => {
                            last_err = None;
                            break;
                        }
                        Err(err) => {
                            last_err = Some(err.into());
                        }
                    }
                }
                if let Some(err) = last_err {
                    // Trim successfully flushed events so a retry won't re-append them.
                    self.outbox.drain(..n);
                    return Err(err);
                }
            }
            n = end;
        }
        self.outbox.clear();
        Ok(n)
    }

    /// 对知识库运行 gap 扫描，返回检测到的知识缺口。
    ///
    /// 委托给 [`crate::gap::run_gap_scan`]。调用方负责持久化（save_to_repo / flush_outbox）。
    pub fn run_gap_scan(
        &self,
        viewer_scope: Option<&Scope>,
        low_coverage_threshold: usize,
    ) -> Vec<GapFinding> {
        crate::gap::run_gap_scan(&self.store, viewer_scope, low_coverage_threshold)
    }
}

pub fn collect_basic_lint_findings(
    schema: &DomainSchema,
    store: &InMemoryStore,
    viewer_scope: Option<&Scope>,
) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    let visible = |s: &Scope| match viewer_scope {
        None => true,
        Some(v) => document_visible_to_viewer(s, v),
    };
    for c in store.claims.values() {
        if !visible(&c.scope) {
            continue;
        }
        if c.quality_score < 0.35 {
            findings.push(LintFinding {
                code: "quality.low".into(),
                message: "claim quality below threshold".into(),
                severity: LintSeverity::Warn,
                subject: Some(c.id.0.to_string()),
            });
        }
        if c.stale {
            findings.push(LintFinding {
                code: "lifecycle.stale".into(),
                message: "stale claim retained for audit".into(),
                severity: LintSeverity::Info,
                subject: Some(c.id.0.to_string()),
            });
        }
    }
    let titles: std::collections::HashSet<String> = store
        .pages
        .values()
        .filter(|p| visible(&p.scope))
        .map(|p| p.title.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    let page_links: std::collections::HashMap<_, _> = store
        .pages
        .values()
        .map(|p| (p.id, extract_wikilinks(&p.markdown)))
        .collect();
    for p in store.pages.values() {
        if !visible(&p.scope) {
            continue;
        }
        if p.title.trim().is_empty() {
            findings.push(LintFinding {
                code: "page.empty_title".into(),
                message: "wiki page has empty title".into(),
                severity: LintSeverity::Error,
                subject: Some(p.id.0.to_string()),
            });
        }
        for link in page_links.get(&p.id).into_iter().flatten() {
            if !titles.contains(link) {
                findings.push(LintFinding {
                    code: "page.broken_wikilink".into(),
                    message: format!("broken wikilink: {link}"),
                    severity: LintSeverity::Warn,
                    subject: Some(p.id.0.to_string()),
                });
            }
        }
        findings.extend(wiki_core::quality::check_page_completeness(schema, p));
    }
    let mut inbound_count: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for p in store.pages.values() {
        if !visible(&p.scope) {
            continue;
        }
        for link in page_links.get(&p.id).into_iter().flatten() {
            *inbound_count.entry(link.clone()).or_insert(0) += 1;
        }
    }
    for p in store.pages.values() {
        if !visible(&p.scope) {
            continue;
        }
        if inbound_count.get(&p.title).copied().unwrap_or(0) == 0 {
            findings.push(LintFinding {
                code: "page.orphan".into(),
                message: "page has no inbound wikilinks".into(),
                severity: LintSeverity::Info,
                subject: Some(p.id.0.to_string()),
            });
        }
    }
    let mut page_text = String::new();
    for p in store.pages.values() {
        if !visible(&p.scope) {
            continue;
        }
        page_text.push_str(&p.markdown.to_ascii_lowercase());
        page_text.push('\n');
    }
    for c in store.claims.values() {
        if !visible(&c.scope) {
            continue;
        }
        if c.stale {
            findings.push(LintFinding {
                code: "claim.stale".into(),
                message: "stale claim should be reconciled in wiki pages".into(),
                severity: LintSeverity::Warn,
                subject: Some(c.id.0.to_string()),
            });
        } else if !crate::gap::claim_has_page_reference(&c.text, &page_text) {
            findings.push(LintFinding {
                code: "xref.missing".into(),
                message: "claim keywords are not referenced in current pages".into(),
                severity: LintSeverity::Info,
                subject: Some(c.id.0.to_string()),
            });
        }
    }
    findings
}

/// 自由函数：三路召回 + RRF + 保留强度（不写审计），避免 `SearchPorts` 与 `&mut self` 交错借用。
pub fn query_ranked_with_ports(
    schema: &DomainSchema,
    store: &InMemoryStore,
    ctx: &QueryContext<'_>,
    ports: &dyn SearchPorts,
    now: OffsetDateTime,
    vector_rank_override: Option<Vec<String>>,
    graph_rank_override: Option<Vec<String>>,
) -> Vec<(String, f64)> {
    let lim = ctx.per_stream_limit;
    let bm25 = ports.bm25_ranked_ids(ctx.query, lim);
    let vector = vector_rank_override.unwrap_or_else(|| ports.vector_ranked_ids(ctx.query, lim));
    let graph = graph_rank_override.unwrap_or_else(|| ports.graph_ranked_ids(ctx.query, lim));
    let fused = reciprocal_rank_fusion(&[bm25, vector, graph], ctx.rrf_k);
    rank_fused_with_retention(schema, store, &fused, now)
}

fn rank_fused_with_retention(
    schema: &DomainSchema,
    store: &InMemoryStore,
    fused: &[RankedDoc],
    now: OffsetDateTime,
) -> Vec<(String, f64)> {
    let mut out: Vec<(String, f64)> = fused
        .iter()
        .map(|d| {
            let bonus = if let Some(cid) = parse_claim_doc_id(&d.id) {
                store
                    .claims
                    .get(&cid)
                    .map(|c| {
                        let hl = schema
                            .tier_half_life_days
                            .get(&c.tier)
                            .copied()
                            .unwrap_or(schema.default_retention.half_life_days);
                        let rp = wiki_core::RetentionParams { half_life_days: hl };
                        retention_strength(c, now, rp)
                    })
                    .unwrap_or(1.0)
            } else {
                1.0
            };
            (d.id.clone(), d.rrf_score * bonus)
        })
        .collect();
    out.sort_by(|a, b| a.1.total_cmp(&b.1).reverse().then_with(|| a.0.cmp(&b.0)));
    out
}

fn parse_claim_doc_id(s: &str) -> Option<ClaimId> {
    let rest = s.strip_prefix("claim:")?;
    let u = uuid::Uuid::parse_str(rest).ok()?;
    Some(ClaimId(u))
}

fn contradicts_heuristic(a: &str, b: &str) -> bool {
    let la = a.to_ascii_lowercase();
    let lb = b.to_ascii_lowercase();
    (la.contains("不是") && lb.contains("是"))
        || (lb.contains("不是") && la.contains("是"))
        || (la.contains("cannot") && lb.contains("can "))
        || (lb.contains("cannot") && la.contains("can "))
}

/// 根据 EntryType 和 DomainSchema 计算创建时的初始 EntryStatus：
/// - `entry_type.auto_approved_on_create()` → Approved（Summary / QA / LintReport / Index）
/// - `schema.find_lifecycle_rule(et)` 命中 → 按 rule.initial_status
/// - 无 entry_type 或无对应 rule → Draft
pub fn initial_status_for(entry_type: Option<&EntryType>, schema: &DomainSchema) -> EntryStatus {
    match entry_type {
        None => EntryStatus::Draft,
        Some(et) => {
            if et.auto_approved_on_create() {
                return EntryStatus::Approved;
            }
            schema
                .find_lifecycle_rule(et)
                .map(|r| r.initial_status)
                .unwrap_or(EntryStatus::Draft)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiki_core::{SessionCrystallizationInput, WikiEvent, WikiPage};
    use wiki_storage::{SqliteRepository, WikiRepository};

    fn private_scope() -> Scope {
        Scope::Private {
            agent_id: "a1".into(),
        }
    }

    #[test]
    fn ingest_and_file_claim_flow() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let sid = eng.ingest_raw(
            "file:///tmp/note.md",
            "项目使用 Redis\nAuthorization: Bearer secret",
            Scope::Private {
                agent_id: "a1".into(),
            },
            "tester",
        );
        let cid = eng.file_claim(
            "项目使用 Redis",
            Scope::Private {
                agent_id: "a1".into(),
            },
            MemoryTier::Working,
            "tester",
        );
        eng.attach_sources(cid, &[sid]).unwrap();
        assert!(eng.store.sources[&sid].body.contains("REDACTED"));
        assert!(!eng.store.claims[&cid].source_ids.is_empty());
    }

    #[test]
    fn ingest_raw_with_tags_writes_normalized_source_tags() {
        let mut schema = DomainSchema::permissive_default();
        schema.tag_config.seed_tags = vec!["alpha".into(), "beta".into()];
        let mut eng = LlmWikiEngine::new(schema);

        let sid = eng
            .ingest_raw_with_tags(
                "file:///tmp/note.md",
                "body",
                private_scope(),
                "tester",
                [" Alpha ", "", "beta", "alpha"],
            )
            .unwrap();

        assert_eq!(eng.store.sources[&sid].tags, vec!["Alpha", "beta"]);
    }

    #[test]
    fn file_claim_with_tags_writes_normalized_claim_tags() {
        let mut schema = DomainSchema::permissive_default();
        schema.tag_config.seed_tags = vec!["topic".into(), "owner".into()];
        let mut eng = LlmWikiEngine::new(schema);

        let cid = eng
            .file_claim_with_tags(
                "tagged claim",
                private_scope(),
                MemoryTier::Semantic,
                "tester",
                [" topic ", "owner", "TOPIC"],
            )
            .unwrap();

        assert_eq!(eng.store.claims[&cid].tags, vec!["topic", "owner"]);
    }

    #[test]
    fn deprecated_tag_rejects_source_and_does_not_insert() {
        let mut schema = DomainSchema::permissive_default();
        schema.tag_config.deprecated_tags = vec!["old".into()];
        let mut eng = LlmWikiEngine::new(schema);

        let err = eng
            .ingest_raw_with_tags(
                "file:///tmp/note.md",
                "body",
                private_scope(),
                "tester",
                [" old "],
            )
            .unwrap_err();

        assert!(matches!(
            err,
            EngineError::TagPolicy(TagPolicyError::DeprecatedTag(tag)) if tag == "old"
        ));
        assert!(eng.store.sources.is_empty());
        assert!(eng.audits.is_empty());
        assert!(eng.outbox.is_empty());
    }

    #[test]
    fn deprecated_tag_rejects_claim_and_does_not_insert() {
        let mut schema = DomainSchema::permissive_default();
        schema.tag_config.deprecated_tags = vec!["old".into()];
        let mut eng = LlmWikiEngine::new(schema);

        let err = eng
            .file_claim_with_tags(
                "claim",
                private_scope(),
                MemoryTier::Semantic,
                "tester",
                ["old"],
            )
            .unwrap_err();

        assert!(matches!(
            err,
            EngineError::TagPolicy(TagPolicyError::DeprecatedTag(tag)) if tag == "old"
        ));
        assert!(eng.store.claims.is_empty());
        assert!(eng.audits.is_empty());
        assert!(eng.outbox.is_empty());
    }

    #[test]
    fn max_new_tags_rejects_and_does_not_insert() {
        let mut schema = DomainSchema::permissive_default();
        schema.tag_config.seed_tags = vec!["known".into()];
        schema.tag_config.max_new_tags_per_ingest = 1;
        let mut eng = LlmWikiEngine::new(schema);

        let err = eng
            .file_claim_with_tags(
                "claim",
                private_scope(),
                MemoryTier::Semantic,
                "tester",
                ["known", "new-one", "new-two"],
            )
            .unwrap_err();

        assert!(matches!(
            err,
            EngineError::TagPolicy(TagPolicyError::TooManyNewTags {
                count: 2,
                max: 1,
                ..
            })
        ));
        assert!(eng.store.claims.is_empty());
        assert!(eng.audits.is_empty());
        assert!(eng.outbox.is_empty());
    }

    #[test]
    fn legacy_ingest_raw_and_file_claim_still_use_empty_tags() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());

        let sid = eng.ingest_raw("file:///tmp/note.md", "body", private_scope(), "tester");
        let cid = eng.file_claim("claim", private_scope(), MemoryTier::Working, "tester");

        assert!(eng.store.sources[&sid].tags.is_empty());
        assert!(eng.store.claims[&cid].tags.is_empty());
    }

    #[test]
    fn loads_schema_from_json_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("schema.json");
        let s = DomainSchema::permissive_default();
        std::fs::write(&path, serde_json::to_vec(&s).unwrap()).unwrap();
        let eng = LlmWikiEngine::from_schema_json_path(&path).unwrap();
        assert_eq!(eng.schema.title, "default-permissive");
    }

    #[test]
    fn query_pipeline_finds_claims() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        eng.file_claim(
            "Redis caching for API",
            Scope::Private {
                agent_id: "a".into(),
            },
            MemoryTier::Semantic,
            "t",
        );
        let ctx = QueryContext::new("Redis API")
            .with_per_stream_limit(20)
            .with_viewer_scope(Scope::Private {
                agent_id: "a".into(),
            });
        let now = OffsetDateTime::now_utc();
        let ranked = eng.query_pipeline_memory(&ctx, now, "t", None, None);
        assert!(!ranked.is_empty());
        assert!(ranked[0].0.starts_with("claim:"));
    }

    #[test]
    fn supersede_chain() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let old = eng.file_claim(
            "v1",
            Scope::Shared {
                team_id: "t1".into(),
            },
            MemoryTier::Semantic,
            "u",
        );
        let new = eng
            .supersede(
                old,
                "v2",
                Scope::Shared {
                    team_id: "t1".into(),
                },
                MemoryTier::Semantic,
                "u",
            )
            .unwrap();
        assert!(eng.store.claims[&old].stale);
        assert_eq!(eng.store.claims[&new].supersedes, Some(old));
    }

    #[test]
    fn lint_reports_broken_wikilink() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let p = WikiPage::new(
            "Alpha",
            "Link to [[MissingPage]]",
            Scope::Private {
                agent_id: "a".into(),
            },
        );
        eng.store.pages.insert(p.id, p);
        let findings = eng.run_basic_lint(
            "tester",
            Some(&Scope::Private {
                agent_id: "a".into(),
            }),
        );
        assert!(findings.iter().any(|f| f.code == "page.broken_wikilink"));
    }

    #[test]
    fn lint_treats_hidden_page_title_as_broken_wikilink_for_viewer_scope() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let viewer = Scope::Private {
            agent_id: "a".into(),
        };
        let visible_page = WikiPage::new("Visible", "Link to [[Hidden]]", viewer.clone());
        let hidden_page = WikiPage::new(
            "Hidden",
            "secret",
            Scope::Private {
                agent_id: "b".into(),
            },
        );
        eng.store.pages.insert(visible_page.id, visible_page);
        eng.store.pages.insert(hidden_page.id, hidden_page);

        let findings = eng.run_basic_lint("tester", Some(&viewer));

        assert!(findings
            .iter()
            .any(|f| f.code == "page.broken_wikilink" && f.message == "broken wikilink: Hidden"));
    }

    #[test]
    fn query_respects_private_scope_isolation() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        eng.file_claim(
            "agent A secret",
            Scope::Private {
                agent_id: "alice".into(),
            },
            MemoryTier::Semantic,
            "t",
        );
        eng.file_claim(
            "agent B secret",
            Scope::Private {
                agent_id: "bob".into(),
            },
            MemoryTier::Semantic,
            "t",
        );
        let ctx = QueryContext::new("secret")
            .with_per_stream_limit(20)
            .with_viewer_scope(Scope::Private {
                agent_id: "alice".into(),
            });
        let now = OffsetDateTime::now_utc();
        let ranked = eng.query_pipeline_memory(&ctx, now, "t", None, None);
        assert_eq!(ranked.len(), 1);
        assert!(ranked[0].0.starts_with("claim:"));
        let cid: uuid::Uuid = ranked[0].0.strip_prefix("claim:").unwrap().parse().unwrap();
        assert_eq!(
            eng.store.claims[&ClaimId(cid)].scope,
            Scope::Private {
                agent_id: "alice".into()
            }
        );
    }

    #[test]
    fn persist_and_reload_snapshot_and_outbox() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();

        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let c1 = eng.file_claim(
            "Postgres is default DB",
            Scope::Shared {
                team_id: "t".into(),
            },
            MemoryTier::Semantic,
            "tester",
        );
        let _c2 = eng
            .supersede(
                c1,
                "SQLite is default DB",
                Scope::Shared {
                    team_id: "t".into(),
                },
                MemoryTier::Semantic,
                "tester",
            )
            .unwrap();
        let _ = eng.query_pipeline_memory(
            &QueryContext::new("Postgres").with_viewer_scope(Scope::Shared {
                team_id: "t".into(),
            }),
            OffsetDateTime::now_utc(),
            "tester",
            None,
            None,
        );
        let n = eng.save_to_repo_and_flush_outbox(&repo).unwrap();
        assert!(n > 0);

        let reloaded =
            LlmWikiEngine::load_from_repo(DomainSchema::permissive_default(), &repo, NoopWikiHook)
                .unwrap();
        assert_eq!(reloaded.store.claims.len(), 2);

        let ndjson = repo.export_outbox_ndjson().unwrap();
        assert!(ndjson.contains("query_served"));
        assert!(ndjson.contains("claim_upserted"));
        assert!(ndjson.contains("claim_superseded"));
    }

    #[test]
    fn crystallize_emits_session_crystallized_event() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let draft = eng
            .crystallize(
                SessionCrystallizationInput {
                    question: "What changed?".into(),
                    findings: vec!["Redis enabled".into()],
                    files_touched: vec!["src/main.rs".into()],
                    lessons: vec!["Prefer explicit config".into()],
                    scope: Scope::Private {
                        agent_id: "a".into(),
                    },
                },
                "tester",
            )
            .unwrap();
        assert!(eng.outbox.iter().any(|event| {
            matches!(
                event,
                WikiEvent::SessionCrystallized { page_id, .. } if *page_id == draft.page.id
            )
        }));
    }

    #[test]
    fn run_basic_lint_emits_lint_run_finished_event() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let page = WikiPage::new(
            "Alpha",
            "Link to [[MissingPage]]",
            Scope::Private {
                agent_id: "a".into(),
            },
        );
        eng.store.pages.insert(page.id, page);
        let findings = eng.run_basic_lint(
            "tester",
            Some(&Scope::Private {
                agent_id: "a".into(),
            }),
        );
        assert!(eng.outbox.iter().any(|event| {
            matches!(
                event,
                WikiEvent::LintRunFinished { findings: emitted, .. } if *emitted == findings.len()
            )
        }));
    }

    #[test]
    fn record_query_emits_query_served_event() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let top = vec!["claim:1".into(), "page:2".into()];
        eng.record_query("redis", top.clone(), "tester");
        assert!(eng.outbox.iter().any(|event| {
            matches!(
                event,
                WikiEvent::QueryServed {
                    query_fingerprint,
                    top_doc_ids,
                    ..
                } if query_fingerprint == "redis" && top_doc_ids == &top
            )
        }));
    }

    // --- initial_status_for 测试 ---

    fn schema_with_concept_rule(initial: EntryStatus) -> DomainSchema {
        use wiki_core::{LifecycleRule, PromotionConditions, PromotionRule};
        let mut schema = DomainSchema::permissive_default();
        schema.lifecycle_rules = vec![LifecycleRule {
            entry_types: vec![EntryType::Concept, EntryType::Entity],
            initial_status: initial,
            promotions: vec![PromotionRule {
                from_status: EntryStatus::Draft,
                to_status: EntryStatus::InReview,
                conditions: PromotionConditions {
                    min_age_days: 0,
                    required_sections: vec![],
                    min_references: 0,
                    cooldown_days: None,
                },
            }],
            stale_days: None,
            auto_cleanup: false,
        }];
        schema
    }

    #[test]
    fn initial_status_none_entry_type_is_draft() {
        let schema = DomainSchema::permissive_default();
        assert_eq!(initial_status_for(None, &schema), EntryStatus::Draft);
    }

    #[test]
    fn initial_status_summary_is_approved() {
        let schema = DomainSchema::permissive_default();
        // Summary 调用 auto_approved_on_create() → Approved，无论有无 rule
        assert_eq!(
            initial_status_for(Some(&EntryType::Summary), &schema),
            EntryStatus::Approved
        );
    }

    #[test]
    fn initial_status_concept_follows_rule() {
        let schema = schema_with_concept_rule(EntryStatus::Draft);
        assert_eq!(
            initial_status_for(Some(&EntryType::Concept), &schema),
            EntryStatus::Draft
        );
    }

    #[test]
    fn initial_status_concept_no_rule_falls_back_to_draft() {
        let schema = DomainSchema::permissive_default(); // 空 lifecycle_rules
        assert_eq!(
            initial_status_for(Some(&EntryType::Concept), &schema),
            EntryStatus::Draft
        );
    }

    // --- promote_page 测试 ---

    fn schema_with_full_promotion() -> DomainSchema {
        use wiki_core::{LifecycleRule, PromotionConditions, PromotionRule};
        let mut schema = DomainSchema::permissive_default();
        schema.lifecycle_rules = vec![LifecycleRule {
            entry_types: vec![EntryType::Concept],
            initial_status: EntryStatus::Draft,
            promotions: vec![PromotionRule {
                from_status: EntryStatus::Draft,
                to_status: EntryStatus::InReview,
                conditions: PromotionConditions {
                    min_age_days: 7,
                    required_sections: vec!["定义".into()],
                    min_references: 2,
                    cooldown_days: Some(3),
                },
            }],
            stale_days: None,
            auto_cleanup: false,
        }];
        schema
    }

    #[test]
    fn promote_page_fails_age_too_young() {
        let mut eng = LlmWikiEngine::new(schema_with_full_promotion());
        let p = WikiPage::new(
            "Test",
            "## 定义\ncontent",
            Scope::Private {
                agent_id: "a".into(),
            },
        )
        .with_entry_type(EntryType::Concept);
        let pid = p.id;
        eng.store.pages.insert(pid, p);
        let now = OffsetDateTime::now_utc();
        let res = eng.promote_page(pid, EntryStatus::InReview, "t", now, false);
        assert!(matches!(res, Err(PromotePageError::AgeTooYoung { .. })));
    }

    #[test]
    fn promote_page_fails_missing_sections() {
        let mut eng = LlmWikiEngine::new(schema_with_full_promotion());
        let mut p = WikiPage::new(
            "Test",
            "no headings here",
            Scope::Private {
                agent_id: "a".into(),
            },
        )
        .with_entry_type(EntryType::Concept);
        // 模拟 10 天前创建
        let past = OffsetDateTime::now_utc() - time::Duration::days(10);
        p.updated_at = past;
        p.created_at = Some(past);
        p.status_entered_at = Some(past);
        let pid = p.id;
        eng.store.pages.insert(pid, p);
        let now = OffsetDateTime::now_utc();
        let res = eng.promote_page(pid, EntryStatus::InReview, "t", now, false);
        assert!(matches!(res, Err(PromotePageError::MissingSections(_))));
    }

    #[test]
    fn promote_page_fails_not_enough_references() {
        let mut eng = LlmWikiEngine::new(schema_with_full_promotion());
        let mut p = WikiPage::new(
            "Test",
            "## 定义\ncontent",
            Scope::Private {
                agent_id: "a".into(),
            },
        )
        .with_entry_type(EntryType::Concept);
        let past = OffsetDateTime::now_utc() - time::Duration::days(10);
        p.updated_at = past;
        p.created_at = Some(past);
        p.status_entered_at = Some(past);
        let pid = p.id;
        eng.store.pages.insert(pid, p);
        let now = OffsetDateTime::now_utc();
        let res = eng.promote_page(pid, EntryStatus::InReview, "t", now, false);
        assert!(matches!(
            res,
            Err(PromotePageError::NotEnoughReferences { .. })
        ));
    }

    #[test]
    fn promote_page_fails_cooldown() {
        use wiki_core::{LifecycleRule, PromotionConditions, PromotionRule};
        let mut eng = LlmWikiEngine::new(schema_with_full_promotion());
        let mut p = WikiPage::new(
            "Test",
            "## 定义\ncontent",
            Scope::Private {
                agent_id: "a".into(),
            },
        )
        .with_entry_type(EntryType::Concept);
        p.updated_at = OffsetDateTime::now_utc() - time::Duration::days(10);
        let pid = p.id;
        let title = p.title.clone();
        eng.store.pages.insert(pid, p);
        // 添加 2 个引用 page
        for i in 0..2u32 {
            let ref_page = WikiPage::new(
                format!("Ref{i}"),
                format!("[[{title}]]"),
                Scope::Private {
                    agent_id: "a".into(),
                },
            );
            eng.store.pages.insert(ref_page.id, ref_page);
        }
        // 但 cooldown_days=3，updated_at 只过了 10 天（>7 days age + >3 days cooldown 都满足）
        // 实际上 age 和 cooldown 都基于 updated_at，10 天 > 3 天 cooldown，所以这不会失败
        // 需要改成：updated_at 在 2 天前（age 不够但 cooldown_days=3 也需要的场景不适用）
        // 改为直接测试 cooldown 独立失败的场景：min_age_days=0 但 cooldown_days=5
        let mut schema = DomainSchema::permissive_default();
        schema.lifecycle_rules = vec![LifecycleRule {
            entry_types: vec![EntryType::Concept],
            initial_status: EntryStatus::Draft,
            promotions: vec![PromotionRule {
                from_status: EntryStatus::Draft,
                to_status: EntryStatus::InReview,
                conditions: PromotionConditions {
                    min_age_days: 0,
                    required_sections: vec![],
                    min_references: 0,
                    cooldown_days: Some(5),
                },
            }],
            stale_days: None,
            auto_cleanup: false,
        }];
        let mut eng2 = LlmWikiEngine::new(schema);
        let mut p2 = WikiPage::new(
            "CooldownTest",
            "body",
            Scope::Private {
                agent_id: "a".into(),
            },
        )
        .with_entry_type(EntryType::Concept);
        // cooldown_days=5，status_entered_at 2 天前 → 应失败
        let past = OffsetDateTime::now_utc() - time::Duration::days(2);
        p2.updated_at = past;
        p2.created_at = Some(past);
        p2.status_entered_at = Some(past);
        let pid2 = p2.id;
        eng2.store.pages.insert(pid2, p2);
        let now = OffsetDateTime::now_utc();
        let res = eng2.promote_page(pid2, EntryStatus::InReview, "t", now, false);
        assert!(matches!(res, Err(PromotePageError::Cooldown { .. })));
    }

    #[test]
    fn promote_page_success() {
        let mut eng = LlmWikiEngine::new(schema_with_full_promotion());
        let mut p = WikiPage::new(
            "Test",
            "## 定义\ncontent",
            Scope::Private {
                agent_id: "a".into(),
            },
        )
        .with_entry_type(EntryType::Concept);
        let past = OffsetDateTime::now_utc() - time::Duration::days(10);
        p.updated_at = past;
        p.created_at = Some(past);
        p.status_entered_at = Some(past);
        let pid = p.id;
        let title = p.title.clone();
        eng.store.pages.insert(pid, p);
        // 添加 2 个引用 page，并刷新 outbound links
        for i in 0..2u32 {
            let mut ref_page = WikiPage::new(
                format!("Ref{i}"),
                format!("[[{title}]]"),
                Scope::Private {
                    agent_id: "a".into(),
                },
            );
            ref_page.refresh_outbound_links();
            eng.store.pages.insert(ref_page.id, ref_page);
        }
        let now = OffsetDateTime::now_utc();
        eng.promote_page(pid, EntryStatus::InReview, "t", now, false)
            .unwrap();
        assert_eq!(eng.store.pages[&pid].status, EntryStatus::InReview);
    }

    #[test]
    fn promote_page_age_is_from_created_at_not_updated_at() {
        // min_age=7 days：即使 updated_at 刚刚刷新（模拟一次编辑），只要 created_at
        // 足够久远、其它条件也都满足，晋升仍应通过。这条测试锁住"编辑不重置 age"。
        let mut eng = LlmWikiEngine::new(schema_with_full_promotion());
        let mut p = WikiPage::new(
            "EditAfterOld",
            "## 定义\ncontent",
            Scope::Private {
                agent_id: "a".into(),
            },
        )
        .with_entry_type(EntryType::Concept);
        let now = OffsetDateTime::now_utc();
        p.created_at = Some(now - time::Duration::days(30));
        p.status_entered_at = Some(now - time::Duration::days(30));
        p.updated_at = now; // 刚刚编辑过
        let pid = p.id;
        let title = p.title.clone();
        eng.store.pages.insert(pid, p);
        for i in 0..2u32 {
            let mut ref_page = WikiPage::new(
                format!("Ref{i}"),
                format!("[[{title}]]"),
                Scope::Private {
                    agent_id: "a".into(),
                },
            );
            ref_page.refresh_outbound_links();
            eng.store.pages.insert(ref_page.id, ref_page);
        }
        eng.promote_page(pid, EntryStatus::InReview, "t", now, false)
            .unwrap();
        let page = &eng.store.pages[&pid];
        assert_eq!(page.status, EntryStatus::InReview);
        // 晋升后 status_entered_at 必须同步刷新到 now
        assert_eq!(page.status_entered_at, Some(now));
    }

    #[test]
    fn promote_page_force_skips_conditions() {
        let mut eng = LlmWikiEngine::new(schema_with_full_promotion());
        let p = WikiPage::new(
            "Test",
            "no headings",
            Scope::Private {
                agent_id: "a".into(),
            },
        )
        .with_entry_type(EntryType::Concept);
        let pid = p.id;
        eng.store.pages.insert(pid, p);
        let now = OffsetDateTime::now_utc();
        eng.promote_page(pid, EntryStatus::InReview, "t", now, true)
            .unwrap();
        assert_eq!(eng.store.pages[&pid].status, EntryStatus::InReview);
    }

    // --- mark_stale_pages + cleanup_expired_pages 测试 ---

    fn schema_with_stale_rule(stale_days: u64, auto_cleanup: bool) -> DomainSchema {
        use wiki_core::{LifecycleRule, PromotionConditions, PromotionRule};
        let mut schema = DomainSchema::permissive_default();
        schema.lifecycle_rules = vec![LifecycleRule {
            entry_types: vec![EntryType::Concept],
            initial_status: EntryStatus::Draft,
            promotions: vec![PromotionRule {
                from_status: EntryStatus::Draft,
                to_status: EntryStatus::InReview,
                conditions: PromotionConditions {
                    min_age_days: 0,
                    required_sections: vec![],
                    min_references: 0,
                    cooldown_days: None,
                },
            }],
            stale_days: Some(stale_days),
            auto_cleanup,
        }];
        schema
    }

    #[test]
    fn mark_stale_marks_expired_concept() {
        let mut eng = LlmWikiEngine::new(schema_with_stale_rule(30, false));
        let mut p = WikiPage::new(
            "Old",
            "body",
            Scope::Private {
                agent_id: "a".into(),
            },
        )
        .with_entry_type(EntryType::Concept);
        p.updated_at = OffsetDateTime::now_utc() - time::Duration::days(60);
        eng.store.pages.insert(p.id, p);
        let now = OffsetDateTime::now_utc();
        let count = eng.mark_stale_pages(now);
        assert_eq!(count, 1);
        let pid = eng.store.pages.keys().next().unwrap();
        assert_eq!(eng.store.pages[pid].status, EntryStatus::NeedsUpdate);
    }

    #[test]
    fn mark_stale_emits_page_status_changed_event() {
        let mut eng = LlmWikiEngine::new(schema_with_stale_rule(30, false));
        let mut p = WikiPage::new(
            "Old",
            "body",
            Scope::Private {
                agent_id: "a".into(),
            },
        )
        .with_entry_type(EntryType::Concept);
        p.updated_at = OffsetDateTime::now_utc() - time::Duration::days(60);
        let pid = p.id;
        eng.store.pages.insert(pid, p);
        let now = OffsetDateTime::now_utc();
        assert_eq!(eng.mark_stale_pages(now), 1);
        assert!(eng.outbox.iter().any(|event| {
            matches!(
                event,
                WikiEvent::PageStatusChanged {
                    page_id,
                    from: EntryStatus::Draft,
                    to: EntryStatus::NeedsUpdate,
                    ..
                } if *page_id == pid
            )
        }));
    }

    #[test]
    fn mark_stale_skips_not_expired() {
        let mut eng = LlmWikiEngine::new(schema_with_stale_rule(30, false));
        let p = WikiPage::new(
            "Fresh",
            "body",
            Scope::Private {
                agent_id: "a".into(),
            },
        )
        .with_entry_type(EntryType::Concept);
        eng.store.pages.insert(p.id, p);
        let now = OffsetDateTime::now_utc();
        let count = eng.mark_stale_pages(now);
        assert_eq!(count, 0);
    }

    #[test]
    fn mark_stale_skips_already_needs_update() {
        let mut eng = LlmWikiEngine::new(schema_with_stale_rule(30, false));
        let mut p = WikiPage::new(
            "AlreadyStale",
            "body",
            Scope::Private {
                agent_id: "a".into(),
            },
        )
        .with_entry_type(EntryType::Concept)
        .with_status(EntryStatus::NeedsUpdate);
        p.updated_at = OffsetDateTime::now_utc() - time::Duration::days(60);
        eng.store.pages.insert(p.id, p);
        let now = OffsetDateTime::now_utc();
        let count = eng.mark_stale_pages(now);
        assert_eq!(count, 0);
    }

    #[test]
    fn mark_stale_noop_without_stale_days() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let mut p = WikiPage::new(
            "NoRule",
            "body",
            Scope::Private {
                agent_id: "a".into(),
            },
        )
        .with_entry_type(EntryType::Concept);
        p.updated_at = OffsetDateTime::now_utc() - time::Duration::days(999);
        eng.store.pages.insert(p.id, p);
        let now = OffsetDateTime::now_utc();
        let count = eng.mark_stale_pages(now);
        assert_eq!(count, 0);
    }

    // --- auto_cleanup 测试 ---

    #[test]
    fn cleanup_removes_expired_autocleanup_page() {
        let mut eng = LlmWikiEngine::new(schema_with_stale_rule(10, true));
        let mut p = WikiPage::new(
            "LintReport",
            "body",
            Scope::Private {
                agent_id: "a".into(),
            },
        )
        .with_entry_type(EntryType::Concept);
        p.updated_at = OffsetDateTime::now_utc() - time::Duration::days(30);
        let pid = p.id;
        eng.store.pages.insert(pid, p);
        let now = OffsetDateTime::now_utc();
        let count = eng.cleanup_expired_pages(now);
        assert_eq!(count, 1);
        assert!(!eng.store.pages.contains_key(&pid));
    }

    #[test]
    fn cleanup_expired_pages_emits_page_deleted_event() {
        let mut eng = LlmWikiEngine::new(schema_with_stale_rule(10, true));
        let mut p = WikiPage::new(
            "LintReport",
            "body",
            Scope::Private {
                agent_id: "a".into(),
            },
        )
        .with_entry_type(EntryType::Concept);
        p.updated_at = OffsetDateTime::now_utc() - time::Duration::days(30);
        let pid = p.id;
        eng.store.pages.insert(pid, p);
        let now = OffsetDateTime::now_utc();
        assert_eq!(eng.cleanup_expired_pages(now), 1);
        assert!(eng.outbox.iter().any(
            |event| matches!(event, WikiEvent::PageDeleted { page_id, .. } if *page_id == pid)
        ));
    }

    #[test]
    fn cleanup_skips_non_autocleanup() {
        let mut eng = LlmWikiEngine::new(schema_with_stale_rule(10, false));
        let mut p = WikiPage::new(
            "Concept",
            "body",
            Scope::Private {
                agent_id: "a".into(),
            },
        )
        .with_entry_type(EntryType::Concept);
        p.updated_at = OffsetDateTime::now_utc() - time::Duration::days(30);
        let pid = p.id;
        eng.store.pages.insert(pid, p);
        let now = OffsetDateTime::now_utc();
        let count = eng.cleanup_expired_pages(now);
        assert_eq!(count, 0);
        assert!(eng.store.pages.contains_key(&pid));
    }

    #[test]
    fn cleanup_skips_not_expired() {
        let mut eng = LlmWikiEngine::new(schema_with_stale_rule(100, true));
        let p = WikiPage::new(
            "Fresh",
            "body",
            Scope::Private {
                agent_id: "a".into(),
            },
        )
        .with_entry_type(EntryType::Concept);
        let pid = p.id;
        eng.store.pages.insert(pid, p);
        let now = OffsetDateTime::now_utc();
        let count = eng.cleanup_expired_pages(now);
        assert_eq!(count, 0);
        assert!(eng.store.pages.contains_key(&pid));
    }

    // --- D2 反向 promotion 测试 ---

    /// schema 含反向规则时，NeedsUpdate → Approved 应成功
    #[test]
    fn promote_needs_update_to_approved_works() {
        use wiki_core::{LifecycleRule, PromotionConditions, PromotionRule};
        let mut schema = DomainSchema::permissive_default();
        schema.lifecycle_rules = vec![LifecycleRule {
            entry_types: vec![EntryType::Concept],
            initial_status: EntryStatus::Draft,
            promotions: vec![
                PromotionRule {
                    from_status: EntryStatus::Draft,
                    to_status: EntryStatus::Approved,
                    conditions: PromotionConditions {
                        min_age_days: 0,
                        required_sections: vec![],
                        min_references: 0,
                        cooldown_days: None,
                    },
                },
                // 反向规则
                PromotionRule {
                    from_status: EntryStatus::NeedsUpdate,
                    to_status: EntryStatus::Approved,
                    conditions: PromotionConditions {
                        min_age_days: 0,
                        required_sections: vec![],
                        min_references: 0,
                        cooldown_days: None,
                    },
                },
            ],
            stale_days: Some(7),
            auto_cleanup: false,
        }];
        let mut eng = LlmWikiEngine::new(schema);
        let p = WikiPage::new(
            "Stale Page",
            "body",
            Scope::Private {
                agent_id: "a".into(),
            },
        )
        .with_entry_type(EntryType::Concept)
        .with_status(EntryStatus::NeedsUpdate);
        let pid = p.id;
        eng.store.pages.insert(pid, p);

        let now = OffsetDateTime::now_utc();
        eng.promote_page(pid, EntryStatus::Approved, "t", now, false)
            .unwrap();

        assert_eq!(eng.store.pages[&pid].status, EntryStatus::Approved);
        // PageStatusChanged 事件应被发出（在 outbox 中）
        let event_fired = eng.outbox.iter().any(|ev| {
            matches!(
                ev,
                wiki_core::WikiEvent::PageStatusChanged {
                    from: EntryStatus::NeedsUpdate,
                    to: EntryStatus::Approved,
                    ..
                }
            )
        });
        assert!(
            event_fired,
            "应发出 PageStatusChanged(NeedsUpdate→Approved)"
        );
    }

    /// schema 无反向规则时，NeedsUpdate → Approved 应返回 NoPromotion 错误
    #[test]
    fn promote_needs_update_without_rule_still_errors() {
        use wiki_core::{LifecycleRule, PromotionConditions, PromotionRule};
        let mut schema = DomainSchema::permissive_default();
        // 只有 Draft→InReview，没有 NeedsUpdate→Approved
        schema.lifecycle_rules = vec![LifecycleRule {
            entry_types: vec![EntryType::Concept],
            initial_status: EntryStatus::Draft,
            promotions: vec![PromotionRule {
                from_status: EntryStatus::Draft,
                to_status: EntryStatus::InReview,
                conditions: PromotionConditions {
                    min_age_days: 0,
                    required_sections: vec![],
                    min_references: 0,
                    cooldown_days: None,
                },
            }],
            stale_days: Some(7),
            auto_cleanup: false,
        }];
        let mut eng = LlmWikiEngine::new(schema);
        let p = WikiPage::new(
            "Stale Page",
            "body",
            Scope::Private {
                agent_id: "a".into(),
            },
        )
        .with_entry_type(EntryType::Concept)
        .with_status(EntryStatus::NeedsUpdate);
        let pid = p.id;
        eng.store.pages.insert(pid, p);

        let now = OffsetDateTime::now_utc();
        let result = eng.promote_page(pid, EntryStatus::Approved, "t", now, false);
        assert!(
            matches!(result, Err(PromotePageError::NoPromotion { .. })),
            "无规则时应返回 NoPromotion，got: {result:?}"
        );
    }

    // --- 自定义 SearchPorts 测试 ---

    struct FixedPorts(Vec<String>);

    impl wiki_core::SearchPorts for FixedPorts {
        fn bm25_ranked_ids(&self, _query: &str, _limit: usize) -> Vec<String> {
            self.0.clone()
        }
        fn vector_ranked_ids(&self, _query: &str, _limit: usize) -> Vec<String> {
            self.0.clone()
        }
        fn graph_ranked_ids(&self, _query: &str, _limit: usize) -> Vec<String> {
            self.0.clone()
        }
    }

    #[test]
    fn query_with_custom_ports_uses_provided_ports() {
        let eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let ports = FixedPorts(vec!["claim:fixed-1".into(), "page:fixed-2".into()]);
        let ctx = QueryContext::new("anything")
            .with_per_stream_limit(20)
            .with_viewer_scope(Scope::Private {
                agent_id: "a".into(),
            });
        let now = OffsetDateTime::now_utc();
        let ranked = eng.query_ranked_with_ports(&ctx, now, &ports, None, None);
        assert!(!ranked.is_empty());
        assert_eq!(ranked[0].0, "claim:fixed-1");
    }

    /// query_pipeline_with_ports 使用自定义 SearchPorts 并产生 QueryServed 事件。
    #[test]
    fn query_pipeline_with_ports_works_with_custom_ports() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let ports = FixedPorts(vec!["doc:a".into(), "doc:b".into()]);
        let ctx = QueryContext::new("test")
            .with_per_stream_limit(20)
            .with_viewer_scope(Scope::Private {
                agent_id: "a".into(),
            });
        let now = OffsetDateTime::now_utc();
        let ranked = eng.query_pipeline_with_ports(&ctx, now, "actor", &ports, None, None);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].0, "doc:a");
        assert_eq!(ranked[1].0, "doc:b");
        // 应产生 QueryServed 事件
        let has_event = eng
            .outbox
            .iter()
            .any(|ev| matches!(ev, wiki_core::WikiEvent::QueryServed { .. }));
        assert!(has_event, "应发出 QueryServed 事件");
    }

    #[test]
    fn query_pipeline_memory_backward_compatible() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        eng.file_claim(
            "Redis caching for API",
            Scope::Private {
                agent_id: "a".into(),
            },
            MemoryTier::Semantic,
            "t",
        );
        let ctx = QueryContext::new("Redis API")
            .with_per_stream_limit(20)
            .with_viewer_scope(Scope::Private {
                agent_id: "a".into(),
            });
        let now = OffsetDateTime::now_utc();
        let ranked = eng.query_pipeline_memory(&ctx, now, "t", None, None);
        assert!(!ranked.is_empty());
        assert!(ranked[0].0.starts_with("claim:"));
    }
}
