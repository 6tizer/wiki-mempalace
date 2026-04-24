//! 只读 metrics 报告模型。
//!
//! 本模块只定义可序列化的数据结构，不读取数据库、不触发写入、不要求 schema migration。

use crate::gap::GapSeverity;
use crate::quality::LintSeverity;
use crate::schema::{EntryStatus, EntryType};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WikiMetricsReport {
    pub generated_at: Option<OffsetDateTime>,
    pub content: ContentMetrics,
    pub lint: LintMetrics,
    pub gaps: GapMetrics,
    pub outbox: OutboxMetrics,
    pub lifecycle: LifecycleMetrics,
}

impl WikiMetricsReport {
    pub fn new(generated_at: OffsetDateTime) -> Self {
        Self {
            generated_at: Some(generated_at),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ContentMetrics {
    pub sources: u64,
    pub pages: u64,
    pub claims: u64,
    pub entities: u64,
    pub relations: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LintMetrics {
    pub total_findings: u64,
    pub severity: LintSeverityCounts,
}

impl LintMetrics {
    pub fn add_severity(&mut self, severity: LintSeverity) {
        self.total_findings += 1;
        self.severity.increment(severity);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GapMetrics {
    pub total_findings: u64,
    pub severity: GapSeverityCounts,
}

impl GapMetrics {
    pub fn add_severity(&mut self, severity: GapSeverity) {
        self.total_findings += 1;
        self.severity.increment(severity);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LintSeverityCounts {
    pub info: u64,
    pub warn: u64,
    pub error: u64,
}

impl LintSeverityCounts {
    pub fn increment(&mut self, severity: LintSeverity) {
        match severity {
            LintSeverity::Info => self.info += 1,
            LintSeverity::Warn => self.warn += 1,
            LintSeverity::Error => self.error += 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GapSeverityCounts {
    pub low: u64,
    pub medium: u64,
    pub high: u64,
}

impl GapSeverityCounts {
    pub fn increment(&mut self, severity: GapSeverity) {
        match severity {
            GapSeverity::Low => self.low += 1,
            GapSeverity::Medium => self.medium += 1,
            GapSeverity::High => self.high += 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct OutboxMetrics {
    pub head_id: Option<i64>,
    pub total_events: u64,
    pub unprocessed_events: u64,
    pub consumer_tag: Option<String>,
    pub acked_up_to_id: Option<i64>,
    pub backlog_events: u64,
}

impl OutboxMetrics {
    pub fn for_consumer(
        head_id: Option<i64>,
        total_events: u64,
        unprocessed_events: u64,
        consumer_tag: impl Into<String>,
        acked_up_to_id: Option<i64>,
        backlog_events: u64,
    ) -> Self {
        Self {
            head_id,
            total_events,
            unprocessed_events,
            consumer_tag: Some(consumer_tag.into()),
            acked_up_to_id,
            backlog_events,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LifecycleMetrics {
    pub page_status: Vec<PageStatusCount>,
    pub entry_type: Vec<EntryTypeCount>,
    pub stale_claims: u64,
}

impl LifecycleMetrics {
    pub fn add_page_status(&mut self, status: EntryStatus, count: u64) {
        self.page_status.push(PageStatusCount { status, count });
    }

    pub fn add_entry_type(&mut self, entry_type: EntryType, count: u64) {
        self.entry_type.push(EntryTypeCount { entry_type, count });
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageStatusCount {
    pub status: EntryStatus,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryTypeCount {
    pub entry_type: EntryType,
    pub count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_report_serializes_to_stable_shape() {
        let json = serde_json::to_value(WikiMetricsReport::default()).unwrap();

        assert_eq!(json["generated_at"], serde_json::Value::Null);
        assert_eq!(json["content"]["sources"], 0);
        assert_eq!(json["lint"]["severity"]["warn"], 0);
        assert_eq!(json["gaps"]["severity"]["high"], 0);
        assert_eq!(json["outbox"]["backlog_events"], 0);
        assert_eq!(json["lifecycle"]["stale_claims"], 0);
    }

    #[test]
    fn severity_counters_increment_expected_bucket() {
        let mut lint = LintMetrics::default();
        lint.add_severity(LintSeverity::Warn);
        lint.add_severity(LintSeverity::Error);

        assert_eq!(lint.total_findings, 2);
        assert_eq!(lint.severity.warn, 1);
        assert_eq!(lint.severity.error, 1);

        let mut gaps = GapMetrics::default();
        gaps.add_severity(GapSeverity::High);

        assert_eq!(gaps.total_findings, 1);
        assert_eq!(gaps.severity.high, 1);
    }
}
