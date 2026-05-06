use std::path::Path;

use time::OffsetDateTime;
use wiki_core::{
    document_visible_to_viewer, ClaimId, CompositeSearchPorts, EntityId, FusionConfig, PageId,
    QueryContext, Scope, SourceId,
};
use wiki_kernel::{
    merge_graph_rankings, InMemorySearchPorts, LlmWikiEngine, NoopWikiHook, SearchPorts,
};
use wiki_mempalace_bridge::MempalaceSearchPorts;
use wiki_storage::{SqliteRepository, SqliteSearchPorts};

pub(crate) fn doc_id_visible_to_viewer(
    doc_id: &str,
    store: &wiki_kernel::InMemoryStore,
    viewer: &Scope,
) -> bool {
    if let Some(rest) = doc_id.strip_prefix("claim:") {
        if let Ok(u) = uuid::Uuid::parse_str(rest) {
            return store
                .claims
                .get(&ClaimId(u))
                .map(|c| document_visible_to_viewer(&c.scope, viewer))
                .unwrap_or(false);
        }
        return false;
    }
    if let Some(rest) = doc_id.strip_prefix("page:") {
        if let Ok(u) = uuid::Uuid::parse_str(rest) {
            return store
                .pages
                .get(&PageId(u))
                .map(|p| document_visible_to_viewer(&p.scope, viewer))
                .unwrap_or(false);
        }
        return false;
    }
    if let Some(rest) = doc_id.strip_prefix("entity:") {
        if let Ok(u) = uuid::Uuid::parse_str(rest) {
            return store
                .entities
                .get(&EntityId(u))
                .map(|e| document_visible_to_viewer(&e.scope, viewer))
                .unwrap_or(false);
        }
        return false;
    }
    if let Some(rest) = doc_id.strip_prefix("source:") {
        if let Ok(u) = uuid::Uuid::parse_str(rest) {
            return store
                .sources
                .get(&SourceId(u))
                .map(|s| document_visible_to_viewer(&s.scope, viewer))
                .unwrap_or(false);
        }
    }
    false
}

pub(crate) fn graph_extra_visible_to_viewer(
    doc_id: &str,
    store: &wiki_kernel::InMemoryStore,
    viewer: &Scope,
) -> bool {
    if doc_id.starts_with("mp_drawer:") || doc_id.starts_with("mp_kg:") {
        return false;
    }
    if doc_id.starts_with("claim:")
        || doc_id.starts_with("page:")
        || doc_id.starts_with("entity:")
        || doc_id.starts_with("source:")
    {
        return doc_id_visible_to_viewer(doc_id, store, viewer);
    }
    false
}

pub(crate) fn merge_optional_graph_extras(
    base_graph: Vec<String>,
    graph_extras: Option<Vec<String>>,
    per_stream_limit: usize,
) -> Vec<String> {
    graph_extras
        .map(|extras| merge_graph_rankings(base_graph.clone(), extras, per_stream_limit))
        .unwrap_or(base_graph)
}

pub(crate) fn filter_graph_extras_for_viewer(
    extras: Vec<String>,
    store: &wiki_kernel::InMemoryStore,
    viewer: &Scope,
) -> Vec<String> {
    extras
        .into_iter()
        .filter(|id| graph_extra_visible_to_viewer(id, store, viewer))
        .collect()
}

pub(crate) fn build_wiki_search_ports<'a>(
    repo: &'a SqliteRepository,
    eng: &'a LlmWikiEngine<NoopWikiHook>,
    viewer: &Scope,
) -> Box<dyn SearchPorts + 'a> {
    match SqliteSearchPorts::open(repo, Some(viewer.clone())) {
        Ok(ports) => Box::new(ports),
        Err(error) => {
            eprintln!(
                "警告：无法创建 storage-backed wiki 搜索端口: {}，回退到 InMemorySearchPorts",
                error
            );
            Box::new(InMemorySearchPorts::new(&eng.store, Some(viewer.clone())))
        }
    }
}

pub(crate) fn run_fusion_query<'a>(
    palace_db: Option<&str>,
    palace_bank: &str,
    repo: &'a SqliteRepository,
    eng: &'a LlmWikiEngine<NoopWikiHook>,
    viewer: &'a Scope,
    ctx: &QueryContext<'_>,
    now: OffsetDateTime,
    vec_override: Option<Vec<String>>,
    graph_extras: Option<Vec<String>>,
) -> Vec<(String, f64)> {
    let wiki_ports = build_wiki_search_ports(repo, eng, viewer);
    let ports: Box<dyn SearchPorts + 'a> = if let Some(pdb) = palace_db {
        match MempalaceSearchPorts::open(Path::new(pdb), Some(palace_bank.to_string())) {
            Ok(mp_ports) => Box::new(CompositeSearchPorts::new(
                vec![wiki_ports, Box::new(mp_ports)],
                FusionConfig::default(),
            )),
            Err(e) => {
                eprintln!(
                    "警告：无法打开 mempalace DB ({}): {}，回退到纯 wiki 检索",
                    pdb, e
                );
                wiki_ports
            }
        }
    } else {
        wiki_ports
    };
    let graph_override = graph_extras.map(|extras| {
        let active_graph =
            SearchPorts::graph_ranked_ids(ports.as_ref(), ctx.query, ctx.per_stream_limit);
        merge_graph_rankings(active_graph, extras, ctx.per_stream_limit)
    });
    eng.query_ranked_with_ports(ctx, now, ports.as_ref(), vec_override, graph_override)
}
