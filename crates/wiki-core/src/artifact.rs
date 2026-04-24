//! 原始资料层：verbatim + 元数据，供 ingest 与结晶回溯。

use crate::model::{Scope, SourceId};
use crate::tags::normalize_tags;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawArtifact {
    pub id: SourceId,
    pub uri: String,
    pub body: String,
    pub scope: Scope,
    #[serde(default)]
    pub tags: Vec<String>,
    pub ingested_at: OffsetDateTime,
}

impl RawArtifact {
    pub fn new(uri: impl Into<String>, body: impl Into<String>, scope: Scope) -> Self {
        Self {
            id: SourceId(Uuid::new_v4()),
            uri: uri.into(),
            body: body.into(),
            scope,
            tags: Vec::new(),
            ingested_at: OffsetDateTime::now_utc(),
        }
    }

    pub fn with_tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.tags = normalize_tags(tags);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_artifact_old_json_without_tags_deserializes_to_empty_vec() {
        let original = RawArtifact::new(
            "file:///note.md",
            "body",
            Scope::Private {
                agent_id: "cli".into(),
            },
        );
        let mut value = serde_json::to_value(original).unwrap();
        value.as_object_mut().unwrap().remove("tags");

        let artifact: RawArtifact = serde_json::from_value(value).unwrap();
        assert!(artifact.tags.is_empty());
    }

    #[test]
    fn raw_artifact_new_initializes_empty_tags_and_builder_normalizes() {
        let artifact = RawArtifact::new(
            "file:///note.md",
            "body",
            Scope::Private {
                agent_id: "cli".into(),
            },
        );
        assert!(artifact.tags.is_empty());

        let tagged = artifact.with_tags([" tag ", "", "tag", "other"]);
        assert_eq!(tagged.tags, vec!["tag", "other"]);
    }
}
