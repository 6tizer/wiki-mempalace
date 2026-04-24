//! 标签规范化与 schema 约束。

use crate::schema::DomainSchema;
use std::collections::HashSet;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TagPolicyError {
    #[error("deprecated tag: {0}")]
    DeprecatedTag(String),
    #[error("too many new tags: count={count}, max={max}, tags={tags:?}")]
    TooManyNewTags {
        count: usize,
        max: usize,
        tags: Vec<String>,
    },
}

pub fn normalize_tags<I, S>(tags: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();

    for tag in tags {
        let tag = tag.as_ref().trim();
        if tag.is_empty() {
            continue;
        }
        if seen.insert(tag.to_lowercase()) {
            normalized.push(tag.to_string());
        }
    }

    normalized
}

pub fn validate_tags_against_schema(
    tags: &[String],
    schema: &DomainSchema,
) -> Result<(), TagPolicyError> {
    let cfg = schema.tag_config();
    let deprecated: HashSet<String> = cfg
        .deprecated_tags
        .iter()
        .map(|tag| tag.trim().to_lowercase())
        .collect();
    let seed: HashSet<String> = cfg
        .seed_tags
        .iter()
        .map(|tag| tag.trim().to_lowercase())
        .collect();

    for tag in tags {
        if deprecated.contains(&tag.trim().to_lowercase()) {
            return Err(TagPolicyError::DeprecatedTag(tag.clone()));
        }
    }

    let new_tags = tags
        .iter()
        .filter(|tag| {
            let key = tag.trim().to_lowercase();
            !seed.contains(&key) && !deprecated.contains(&key)
        })
        .cloned()
        .collect::<Vec<_>>();
    let max = cfg.max_new_tags_per_ingest as usize;
    if new_tags.len() > max {
        return Err(TagPolicyError::TooManyNewTags {
            count: new_tags.len(),
            max,
            tags: new_tags,
        });
    }

    Ok(())
}

pub fn normalize_and_validate_tags<I, S>(
    tags: I,
    schema: &DomainSchema,
) -> Result<Vec<String>, TagPolicyError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let tags = normalize_tags(tags);
    validate_tags_against_schema(&tags, schema)?;
    Ok(tags)
}

pub fn normalize_and_validate_tag_groups(
    groups: &[&[String]],
    schema: &DomainSchema,
) -> Result<Vec<Vec<String>>, TagPolicyError> {
    let normalized_groups = groups
        .iter()
        .map(|group| normalize_tags(group.iter().map(String::as_str)))
        .collect::<Vec<_>>();

    let mut seen = HashSet::new();
    let mut merged = Vec::new();
    for group in &normalized_groups {
        for tag in group {
            if seen.insert(tag.to_lowercase()) {
                merged.push(tag.clone());
            }
        }
    }

    validate_tags_against_schema(&merged, schema)?;
    Ok(normalized_groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_trims_drops_empty_dedups_and_preserves_order() {
        let tags = normalize_tags([" Alpha ", "", "beta", "alpha", " gamma ", "beta"]);
        assert_eq!(tags, vec!["Alpha", "beta", "gamma"]);
    }

    #[test]
    fn deprecated_tag_errors_case_insensitively() {
        let mut schema = DomainSchema::permissive_default();
        schema.tag_config.deprecated_tags = vec!["OldTag".into()];

        let err = normalize_and_validate_tags([" oldtag "], &schema).unwrap_err();
        assert_eq!(err, TagPolicyError::DeprecatedTag("oldtag".into()));
    }

    #[test]
    fn max_new_tags_per_ingest_counts_only_non_seed_tags_case_insensitively() {
        let mut schema = DomainSchema::permissive_default();
        schema.tag_config.seed_tags = vec!["Known".into()];
        schema.tag_config.max_new_tags_per_ingest = 1;

        assert!(normalize_and_validate_tags(["known", "new-one"], &schema).is_ok());

        let err =
            normalize_and_validate_tags(["KNOWN", "new-one", "new-two"], &schema).unwrap_err();
        assert_eq!(
            err,
            TagPolicyError::TooManyNewTags {
                count: 2,
                max: 1,
                tags: vec!["new-one".into(), "new-two".into()],
            }
        );
    }

    #[test]
    fn tag_groups_apply_max_new_tags_across_whole_ingest() {
        let mut schema = DomainSchema::permissive_default();
        schema.tag_config.max_new_tags_per_ingest = 1;
        let source_tags = vec!["new-source".to_string()];
        let claim_tags = vec!["new-claim".to_string()];

        let err =
            normalize_and_validate_tag_groups(&[&source_tags, &claim_tags], &schema).unwrap_err();

        assert_eq!(
            err,
            TagPolicyError::TooManyNewTags {
                count: 2,
                max: 1,
                tags: vec!["new-source".into(), "new-claim".into()],
            }
        );
    }

    #[test]
    fn tag_groups_ignore_seed_and_case_duplicates_for_new_count() {
        let mut schema = DomainSchema::permissive_default();
        schema.tag_config.seed_tags = vec!["Known".into()];
        schema.tag_config.max_new_tags_per_ingest = 1;
        let source_tags = vec![" known ".to_string(), "New".to_string()];
        let claim_tags = vec!["KNOWN".to_string(), "new".to_string()];

        let normalized =
            normalize_and_validate_tag_groups(&[&source_tags, &claim_tags], &schema).unwrap();

        assert_eq!(
            normalized,
            vec![
                vec!["known".to_string(), "New".to_string()],
                vec!["KNOWN".to_string(), "new".to_string()]
            ]
        );
    }
}
