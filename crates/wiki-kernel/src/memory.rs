use std::collections::HashMap;
use wiki_storage::StorageSnapshot;

use wiki_core::{
    Claim, ClaimId, Entity, EntityId, GraphSnapshot, PageId, RawArtifact, SourceId, TypedEdge,
    WikiPage,
};

#[derive(Debug, Default)]
pub struct InMemoryStore {
    pub sources: HashMap<SourceId, RawArtifact>,
    pub claims: HashMap<ClaimId, Claim>,
    pub pages: HashMap<PageId, WikiPage>,
    pub entities: HashMap<EntityId, Entity>,
    pub edges: Vec<TypedEdge>,
}

impl InMemoryStore {
    pub fn graph_snapshot(&self) -> GraphSnapshot {
        GraphSnapshot {
            edges: self
                .edges
                .iter()
                .map(|e| (e.from, e.to, e.relation.clone()))
                .collect(),
        }
    }

    pub fn from_snapshot(s: StorageSnapshot) -> Self {
        Self {
            sources: s.sources.into_iter().map(|x| (x.id, x)).collect(),
            claims: s.claims.into_iter().map(|x| (x.id, x)).collect(),
            pages: s.pages.into_iter().map(|x| (x.id, x)).collect(),
            entities: s.entities.into_iter().map(|x| (x.id, x)).collect(),
            edges: s.edges,
        }
    }

    pub fn to_snapshot(&self, audits: &[wiki_core::AuditRecord]) -> StorageSnapshot {
        let mut sources: Vec<_> = self.sources.values().cloned().collect();
        sources.sort_by_key(|x| x.id.0);
        let mut claims: Vec<_> = self.claims.values().cloned().collect();
        claims.sort_by_key(|x| x.id.0);
        let mut pages: Vec<_> = self.pages.values().cloned().collect();
        pages.sort_by_key(|x| x.id.0);
        let mut entities: Vec<_> = self.entities.values().cloned().collect();
        entities.sort_by_key(|x| x.id.0);
        StorageSnapshot {
            sources,
            claims,
            pages,
            entities,
            edges: self.edges.clone(),
            audits: audits.to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiki_core::{MemoryTier, Scope};

    fn shared_scope() -> Scope {
        Scope::Shared {
            team_id: "test".into(),
        }
    }

    #[test]
    fn to_snapshot_is_deterministic() {
        let mut store = InMemoryStore::default();
        let scope = shared_scope();
        for i in 0..8 {
            let claim = Claim::new(
                format!("claim {i}").as_str(),
                scope.clone(),
                MemoryTier::Semantic,
            );
            store.claims.insert(claim.id, claim);
            let page = WikiPage::new(format!("page {i}").as_str(), "body", scope.clone());
            store.pages.insert(page.id, page);
            let src = RawArtifact::new(
                format!("file:///note{i}.md").as_str(),
                format!("body {i}").as_str(),
                scope.clone(),
            );
            store.sources.insert(src.id, src);
        }
        let snap1 = store.to_snapshot(&[]);
        let snap2 = store.to_snapshot(&[]);
        let json1 = serde_json::to_string(&snap1).expect("serialize snap1");
        let json2 = serde_json::to_string(&snap2).expect("serialize snap2");
        assert_eq!(
            json1, json2,
            "to_snapshot must produce identical JSON on repeated calls"
        );
        // Verify ordering is by id ascending
        for i in 1..snap1.claims.len() {
            assert!(
                snap1.claims[i - 1].id.0 <= snap1.claims[i].id.0,
                "claims must be sorted by id"
            );
        }
        for i in 1..snap1.pages.len() {
            assert!(
                snap1.pages[i - 1].id.0 <= snap1.pages[i].id.0,
                "pages must be sorted by id"
            );
        }
    }
}
