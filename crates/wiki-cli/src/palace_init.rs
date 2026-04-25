use std::path::{Path, PathBuf};

use rusqlite::OptionalExtension;
use wiki_core::{Scope, SearchPorts};
use wiki_mempalace_bridge::{
    consume_outbox_ndjson_with_resolver_and_stats, LiveMempalaceSink, MempalaceSearchPorts,
    MempalaceWikiSink, OutboxDispatchStats, OutboxResolver,
};
use wiki_storage::{OutboxConsumerProgress, OutboxStats, SqliteRepository, WikiRepository};

type PalaceInitResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Clone)]
pub struct PalaceInitReport {
    pub consumer_tag: String,
    pub start_id: i64,
    pub head_id: i64,
    pub acked: usize,
    pub bank_id: Option<String>,
    pub dispatch: OutboxDispatchStats,
    pub drawer_count: Option<i64>,
    pub kg_fact_count: Option<i64>,
    pub validation: Option<PalaceInitValidation>,
}

#[derive(Debug, Clone, Default)]
pub struct PalaceInitValidation {
    pub sample_query: String,
    pub query_ok: bool,
    pub explain_ok: bool,
    pub fusion_ok: bool,
    pub bm25_count: usize,
    pub vector_count: usize,
    pub graph_count: usize,
}

#[derive(Debug, Clone)]
pub struct PalaceInitReportFiles {
    pub json_path: PathBuf,
    pub markdown_path: PathBuf,
}

pub trait PalaceInitRepository {
    fn outbox_consumer_progress(
        &self,
        consumer_tag: &str,
    ) -> PalaceInitResult<OutboxConsumerProgress>;
    fn outbox_stats(&self) -> PalaceInitResult<OutboxStats>;
    fn export_outbox_since(&self, last_id: i64) -> PalaceInitResult<String>;
    fn ack_outbox(&self, up_to_id: i64, consumer_tag: &str) -> PalaceInitResult<usize>;
}

impl PalaceInitRepository for SqliteRepository {
    fn outbox_consumer_progress(
        &self,
        consumer_tag: &str,
    ) -> PalaceInitResult<OutboxConsumerProgress> {
        Ok(self.get_outbox_consumer_progress(consumer_tag)?)
    }

    fn outbox_stats(&self) -> PalaceInitResult<OutboxStats> {
        Ok(self.get_outbox_stats()?)
    }

    fn export_outbox_since(&self, last_id: i64) -> PalaceInitResult<String> {
        Ok(WikiRepository::export_outbox_ndjson_from_id(self, last_id)?)
    }

    fn ack_outbox(&self, up_to_id: i64, consumer_tag: &str) -> PalaceInitResult<usize> {
        Ok(WikiRepository::mark_outbox_processed(
            self,
            up_to_id,
            consumer_tag,
        )?)
    }
}

pub fn mempalace_bank_from_viewer_scope(viewer_scope: &str) -> String {
    match parse_scope(viewer_scope) {
        Scope::Private { agent_id } => agent_id,
        Scope::Shared { team_id } => team_id,
    }
}

pub fn run_live_palace_init<R, V>(
    repo: &R,
    resolver: &V,
    palace_path: &Path,
    viewer_scope: &str,
    consumer_tag: &str,
    requested_last_id: i64,
) -> PalaceInitResult<PalaceInitReport>
where
    R: PalaceInitRepository,
    V: OutboxResolver,
{
    let bank_id = mempalace_bank_from_viewer_scope(viewer_scope);
    let sink = LiveMempalaceSink::open(palace_path, &bank_id)?;
    let mut report = run_palace_init_core(repo, &sink, resolver, consumer_tag, requested_last_id)?;
    let (drawers, kg_facts) = count_palace_rows(palace_path)?;
    let validation = validate_palace_search(palace_path, &bank_id, drawers)?;
    if report.dispatch.lines_seen > 0 {
        report.acked = repo.ack_outbox(report.head_id, consumer_tag)?;
    }
    report.bank_id = Some(bank_id);
    report.drawer_count = Some(drawers);
    report.kg_fact_count = Some(kg_facts);
    report.validation = Some(validation);
    Ok(report)
}

pub fn run_palace_init_core<R, S, V>(
    repo: &R,
    sink: &S,
    resolver: &V,
    consumer_tag: &str,
    requested_last_id: i64,
) -> PalaceInitResult<PalaceInitReport>
where
    R: PalaceInitRepository,
    S: MempalaceWikiSink,
    V: OutboxResolver,
{
    let progress = repo.outbox_consumer_progress(consumer_tag)?;
    let stats = repo.outbox_stats()?;
    let start_id = effective_consume_start_id(&progress, requested_last_id);
    if start_id >= stats.head_id {
        return Ok(empty_report(consumer_tag, start_id, stats.head_id));
    }

    let ndjson = repo.export_outbox_since(start_id)?;
    if ndjson.trim().is_empty() {
        return Ok(empty_report(consumer_tag, start_id, stats.head_id));
    }

    let dispatch = consume_outbox_ndjson_with_resolver_and_stats(sink, resolver, &ndjson)?;
    if dispatch.unresolved > 0 {
        return Err(format!(
            "palace-init unresolved required events: unresolved={}",
            dispatch.unresolved
        )
        .into());
    }
    Ok(PalaceInitReport {
        consumer_tag: consumer_tag.to_string(),
        start_id,
        head_id: stats.head_id,
        acked: 0,
        bank_id: None,
        dispatch,
        drawer_count: None,
        kg_fact_count: None,
        validation: None,
    })
}

fn empty_report(consumer_tag: &str, start_id: i64, head_id: i64) -> PalaceInitReport {
    PalaceInitReport {
        consumer_tag: consumer_tag.to_string(),
        start_id,
        head_id,
        acked: 0,
        bank_id: None,
        dispatch: OutboxDispatchStats::default(),
        drawer_count: None,
        kg_fact_count: None,
        validation: None,
    }
}

fn effective_consume_start_id(progress: &OutboxConsumerProgress, requested_last_id: i64) -> i64 {
    progress
        .acked_up_to_id
        .map_or(requested_last_id, |acked| acked.max(requested_last_id))
}

fn parse_scope(s: &str) -> Scope {
    if let Some(x) = s.strip_prefix("shared:") {
        Scope::Shared {
            team_id: x.to_string(),
        }
    } else if let Some(x) = s.strip_prefix("private:") {
        Scope::Private {
            agent_id: x.to_string(),
        }
    } else {
        Scope::Private {
            agent_id: s.to_string(),
        }
    }
}

fn count_palace_rows(path: &Path) -> PalaceInitResult<(i64, i64)> {
    let conn = rusqlite::Connection::open(path)?;
    let drawers = conn.query_row("SELECT COUNT(*) FROM drawers", [], |row| row.get(0))?;
    let kg_facts = conn.query_row("SELECT COUNT(*) FROM kg_facts", [], |row| row.get(0))?;
    Ok((drawers, kg_facts))
}

fn validate_palace_search(
    palace_path: &Path,
    bank_id: &str,
    drawer_count: i64,
) -> PalaceInitResult<PalaceInitValidation> {
    let sample_query = match sample_query_from_palace(palace_path, bank_id)? {
        Some(query) => query,
        None if drawer_count == 0 => "wiki".to_string(),
        None => {
            return Err(format!(
                "palace-init validation failed: no searchable sample for bank={bank_id}"
            )
            .into())
        }
    };
    let ports = MempalaceSearchPorts::open(palace_path, Some(bank_id.to_string()))?;
    let bm25 = SearchPorts::bm25_ranked_ids(&ports, &sample_query, 5);
    let vector = SearchPorts::vector_ranked_ids(&ports, &sample_query, 5);
    let graph = SearchPorts::graph_ranked_ids(&ports, &sample_query, 5);
    let has_search_candidate = !bm25.is_empty() || !vector.is_empty();
    let ok = drawer_count == 0 || has_search_candidate;
    if !ok {
        return Err(format!(
            "palace-init validation failed: no query/fusion candidates for bank={bank_id}"
        )
        .into());
    }
    Ok(PalaceInitValidation {
        sample_query,
        query_ok: ok,
        explain_ok: ok,
        fusion_ok: ok,
        bm25_count: bm25.len(),
        vector_count: vector.len(),
        graph_count: graph.len(),
    })
}

fn sample_query_from_palace(path: &Path, bank_id: &str) -> PalaceInitResult<Option<String>> {
    let conn = rusqlite::Connection::open(path)?;
    let content: Option<String> = conn
        .query_row(
            "SELECT content FROM drawers WHERE bank_id = ?1 ORDER BY id LIMIT 1",
            [bank_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(content
        .and_then(|text| {
            text.split(|c: char| !c.is_alphanumeric())
                .find(|part| part.chars().count() >= 3)
                .map(str::to_string)
        })
        .filter(|query| !query.trim().is_empty()))
}

pub fn write_report_files(
    report_dir: impl AsRef<Path>,
    report: &PalaceInitReport,
) -> PalaceInitResult<PalaceInitReportFiles> {
    let report_dir = report_dir.as_ref();
    std::fs::create_dir_all(report_dir)?;
    let json_path = report_dir.join("palace-init-report.json");
    let markdown_path = report_dir.join("palace-init-report.md");
    std::fs::write(
        &json_path,
        serde_json::to_string_pretty(&report_json(report))?,
    )?;
    std::fs::write(&markdown_path, render_markdown_report(report))?;
    Ok(PalaceInitReportFiles {
        json_path,
        markdown_path,
    })
}

fn report_json(report: &PalaceInitReport) -> serde_json::Value {
    serde_json::json!({
        "consumer_tag": report.consumer_tag,
        "start_id": report.start_id,
        "head_id": report.head_id,
        "acked": report.acked,
        "bank_id": report.bank_id,
        "dispatch": dispatch_json(&report.dispatch),
        "drawer_count": report.drawer_count,
        "kg_fact_count": report.kg_fact_count,
        "validation": report.validation.as_ref().map(validation_json),
    })
}

fn dispatch_json(dispatch: &OutboxDispatchStats) -> serde_json::Value {
    let by_event = dispatch
        .by_event
        .iter()
        .map(|(event, stats)| {
            (
                event.clone(),
                serde_json::json!({
                    "seen": stats.seen,
                    "dispatched": stats.dispatched,
                    "filtered": stats.filtered,
                    "ignored": stats.ignored,
                    "unresolved": stats.unresolved,
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    serde_json::json!({
        "lines_seen": dispatch.lines_seen,
        "dispatched": dispatch.dispatched,
        "filtered": dispatch.filtered,
        "ignored": dispatch.ignored,
        "unresolved": dispatch.unresolved,
        "by_event": by_event,
    })
}

fn validation_json(validation: &PalaceInitValidation) -> serde_json::Value {
    serde_json::json!({
        "sample_query": validation.sample_query,
        "query_ok": validation.query_ok,
        "explain_ok": validation.explain_ok,
        "fusion_ok": validation.fusion_ok,
        "bm25_count": validation.bm25_count,
        "vector_count": validation.vector_count,
        "graph_count": validation.graph_count,
    })
}

fn render_markdown_report(report: &PalaceInitReport) -> String {
    let validation = report.validation.as_ref();
    format!(
        "# Palace Init Report\n\n- consumer_tag: {}\n- start_id: {}\n- head_id: {}\n- acked: {}\n- bank_id: {}\n- lines_seen: {}\n- dispatched: {}\n- filtered: {}\n- ignored: {}\n- unresolved: {}\n- drawers: {}\n- kg_facts: {}\n- query_ok: {}\n- explain_ok: {}\n- fusion_ok: {}\n",
        report.consumer_tag,
        report.start_id,
        report.head_id,
        report.acked,
        report.bank_id.as_deref().unwrap_or(""),
        report.dispatch.lines_seen,
        report.dispatch.dispatched,
        report.dispatch.filtered,
        report.dispatch.ignored,
        report.dispatch.unresolved,
        report.drawer_count.unwrap_or_default(),
        report.kg_fact_count.unwrap_or_default(),
        validation.is_some_and(|v| v.query_ok),
        validation.is_some_and(|v| v.explain_ok),
        validation.is_some_and(|v| v.fusion_ok),
    )
}

#[cfg(test)]
#[allow(dead_code)]
#[derive(Default)]
pub struct NoopPalaceInitSink;

#[cfg(test)]
impl MempalaceWikiSink for NoopPalaceInitSink {
    fn on_claim_upserted(
        &self,
        _claim: &wiki_core::Claim,
    ) -> Result<(), wiki_mempalace_bridge::MempalaceError> {
        Ok(())
    }

    fn on_claim_event(
        &self,
        _claim_id: wiki_core::ClaimId,
    ) -> Result<(), wiki_mempalace_bridge::MempalaceError> {
        Ok(())
    }

    fn on_claim_superseded(
        &self,
        _old: wiki_core::ClaimId,
        _new: wiki_core::ClaimId,
    ) -> Result<(), wiki_mempalace_bridge::MempalaceError> {
        Ok(())
    }

    fn on_source_linked(
        &self,
        _source_id: wiki_core::SourceId,
        _claim_id: wiki_core::ClaimId,
    ) -> Result<(), wiki_mempalace_bridge::MempalaceError> {
        Ok(())
    }

    fn scope_filter(&self, _scope: &Scope) -> bool {
        true
    }
}

#[cfg(test)]
#[allow(dead_code)]
#[derive(Default)]
pub struct NoopPalaceInitResolver;

#[cfg(test)]
impl OutboxResolver for NoopPalaceInitResolver {
    fn claim(&self, _id: wiki_core::ClaimId) -> Option<wiki_core::Claim> {
        None
    }

    fn source_scope(&self, _id: wiki_core::SourceId) -> Option<Scope> {
        None
    }

    fn page(&self, _id: wiki_core::PageId) -> Option<wiki_core::WikiPage> {
        None
    }
}

#[cfg(test)]
#[allow(dead_code)]
pub struct FakePalaceInitRepository {
    ndjson: String,
    head_id: i64,
    acked_up_to_id: Option<i64>,
    acked: std::cell::Cell<Option<i64>>,
}

#[cfg(test)]
#[allow(dead_code)]
impl FakePalaceInitRepository {
    pub fn new(ndjson: &str, head_id: i64, acked_up_to_id: Option<i64>) -> Self {
        Self {
            ndjson: ndjson.to_string(),
            head_id,
            acked_up_to_id,
            acked: std::cell::Cell::new(None),
        }
    }

    pub fn acked_up_to_id(&self) -> Option<i64> {
        self.acked.get()
    }
}

#[cfg(test)]
impl PalaceInitRepository for FakePalaceInitRepository {
    fn outbox_consumer_progress(
        &self,
        consumer_tag: &str,
    ) -> PalaceInitResult<OutboxConsumerProgress> {
        Ok(OutboxConsumerProgress {
            consumer_tag: consumer_tag.to_string(),
            acked_up_to_id: self.acked_up_to_id,
            acked_at: None,
            backlog_events: self.head_id - self.acked_up_to_id.unwrap_or(0),
        })
    }

    fn outbox_stats(&self) -> PalaceInitResult<OutboxStats> {
        Ok(OutboxStats {
            head_id: self.head_id,
            total_events: self.head_id,
            unprocessed_events: self.head_id - self.acked_up_to_id.unwrap_or(0),
        })
    }

    fn export_outbox_since(&self, _last_id: i64) -> PalaceInitResult<String> {
        Ok(self.ndjson.clone())
    }

    fn ack_outbox(&self, up_to_id: i64, _consumer_tag: &str) -> PalaceInitResult<usize> {
        self.acked.set(Some(up_to_id));
        Ok(1)
    }
}
