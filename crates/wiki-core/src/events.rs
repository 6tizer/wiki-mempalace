//! 事件驱动自动化：ingest / session / query / 定时任务 的钩子载荷。

use crate::model::{ClaimId, EntityId, PageId, Scope, SourceId};
use crate::schema::EntryStatus;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

pub const QUERY_SERVED_SCHEMA_VERSION: u32 = 2;
pub const QUERY_HASH_SCHEMA_VERSION: u32 = 1;
const QUERY_HASH_SALT: &str = "wiki-mempalace:query-served:v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WikiEvent {
    SourceIngested {
        source_id: SourceId,
        redacted: bool,
        at: OffsetDateTime,
    },
    ClaimUpserted {
        claim_id: ClaimId,
        at: OffsetDateTime,
    },
    ClaimSuperseded {
        old: ClaimId,
        new: ClaimId,
        at: OffsetDateTime,
    },
    PageWritten {
        page_id: PageId,
        at: OffsetDateTime,
    },
    QueryServed {
        /// Legacy field kept for older readers. New events store the v1 query
        /// hash here, never the raw query.
        query_fingerprint: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        query_hash: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        query_hash_schema_version: Option<u32>,
        #[serde(default = "legacy_query_served_schema_version")]
        schema_version: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        viewer_scope: Option<Scope>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        redacted_preview: Option<String>,
        top_doc_ids: Vec<String>,
        at: OffsetDateTime,
    },
    SessionCrystallized {
        page_id: PageId,
        at: OffsetDateTime,
    },
    GraphExpanded {
        seeds: Vec<EntityId>,
        visited: Vec<EntityId>,
        at: OffsetDateTime,
    },
    LintRunFinished {
        findings: usize,
        at: OffsetDateTime,
    },
    /// 页面生命周期状态变更（promote_page / mark_stale）
    PageStatusChanged {
        page_id: PageId,
        from: EntryStatus,
        to: EntryStatus,
        actor: String,
        at: OffsetDateTime,
    },
    /// 页面因 auto_cleanup 被删除
    PageDeleted {
        page_id: PageId,
        at: OffsetDateTime,
    },
}

impl WikiEvent {
    pub fn query_served(
        query: impl AsRef<str>,
        viewer_scope: Option<Scope>,
        top_doc_ids: Vec<String>,
        at: OffsetDateTime,
    ) -> Self {
        let query_hash = query_hash_v1(query.as_ref());
        Self::QueryServed {
            query_fingerprint: query_hash.clone(),
            query_hash: Some(query_hash),
            query_hash_schema_version: Some(QUERY_HASH_SCHEMA_VERSION),
            schema_version: QUERY_SERVED_SCHEMA_VERSION,
            viewer_scope,
            redacted_preview: None,
            top_doc_ids,
            at,
        }
    }

    /// Build a v1-compatible event shape for old NDJSON fixtures and migration
    /// tests. Production query paths should use `query_served`.
    pub fn legacy_query_served(
        query_fingerprint: impl Into<String>,
        top_doc_ids: Vec<String>,
        at: OffsetDateTime,
    ) -> Self {
        Self::QueryServed {
            query_fingerprint: query_fingerprint.into(),
            query_hash: None,
            query_hash_schema_version: None,
            schema_version: 1,
            viewer_scope: None,
            redacted_preview: None,
            top_doc_ids,
            at,
        }
    }
}

pub fn query_hash_v1(query: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(QUERY_HASH_SALT.as_bytes());
    hasher.update([0]);
    hasher.update(query.as_bytes());
    let digest = hasher.finalize();
    format!("sha256:v1:{}", hex_lower(&digest))
}

fn legacy_query_served_schema_version() -> u32 {
    1
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_served_serializes_hash_without_raw_query() {
        let at = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let event = WikiEvent::query_served(
            "secret customer query",
            Some(Scope::Private {
                agent_id: "agent-a".into(),
            }),
            vec!["page:1".into()],
            at,
        );

        let json = serde_json::to_string(&event).unwrap();

        assert!(!json.contains("secret customer query"));
        assert!(json.contains("\"schema_version\":2"));
        assert!(json.contains("\"query_hash_schema_version\":1"));
        assert!(json.contains("\"query_hash\":\"sha256:v1:"));
        assert!(json.contains("\"viewer_scope\""));
    }

    #[test]
    fn legacy_query_served_deserializes_without_new_fields() {
        let at = serde_json::to_value(OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap())
            .unwrap();
        let legacy = serde_json::json!({
            "type": "query_served",
            "query_fingerprint": "raw legacy query",
            "top_doc_ids": ["page:1"],
            "at": at
        });

        let event: WikiEvent = serde_json::from_value(legacy).unwrap();

        let WikiEvent::QueryServed {
            query_fingerprint,
            query_hash,
            query_hash_schema_version,
            schema_version,
            viewer_scope,
            ..
        } = event
        else {
            panic!("expected query event");
        };
        assert_eq!(query_fingerprint, "raw legacy query");
        assert_eq!(query_hash, None);
        assert_eq!(query_hash_schema_version, None);
        assert_eq!(schema_version, 1);
        assert_eq!(viewer_scope, None);
    }
}
