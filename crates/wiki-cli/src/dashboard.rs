use crate::{
    automation_health_level_name, entry_status_name, entry_type_name, format_automation_record,
    format_automation_time, format_optional_i64, format_outbox_consumer_progress,
    format_outbox_stats, AutomationHealthReport,
};
use wiki_core::WikiMetricsReport;

pub(crate) fn render_dashboard_html(
    health: &AutomationHealthReport,
    metrics: &WikiMetricsReport,
    consumer_tag: &str,
) -> String {
    let status = automation_health_level_name(health.level);
    let generated_at = metrics
        .generated_at
        .map(format_automation_time)
        .unwrap_or_else(|| "unknown".to_string());
    let metrics_consumer_tag = metrics.outbox.consumer_tag.as_deref().unwrap_or("none");
    let page_status = metrics
        .lifecycle
        .page_status
        .iter()
        .map(|item| format!("{}={}", entry_status_name(item.status), item.count))
        .collect::<Vec<_>>()
        .join(" ");
    let entry_type = metrics
        .lifecycle
        .entry_type
        .iter()
        .map(|item| format!("{}={}", entry_type_name(&item.entry_type), item.count))
        .collect::<Vec<_>>()
        .join(" ");

    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n");
    html.push_str("<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    html.push_str("<title>Wiki Dashboard</title>\n");
    html.push_str("<style>\n");
    html.push_str(
        "body{margin:0;font-family:system-ui,-apple-system,Segoe UI,sans-serif;background:#f6f7f9;color:#17202a;}\n",
    );
    html.push_str(
        "main{max-width:1120px;margin:0 auto;padding:32px 20px 48px;}section{background:#fff;border:1px solid #d8dee6;border-radius:8px;margin:16px 0;padding:18px;}\n",
    );
    html.push_str(
        "h1{font-size:28px;margin:0 0 8px;}h2{font-size:18px;margin:0 0 12px;}dl{display:grid;grid-template-columns:minmax(160px,240px) 1fr;gap:8px 14px;margin:0;}dt{font-weight:700;}dd{margin:0;}ul{margin:0;padding-left:20px;}li{margin:6px 0;}.status{font-weight:800;}.status-green{color:#0b7a35;}.status-yellow{color:#946200;}.status-red{color:#b42318;}code{background:#eef1f5;border-radius:4px;padding:1px 4px;}\n",
    );
    html.push_str("</style>\n</head>\n<body>\n<main>\n");
    html.push_str("<h1>Wiki Dashboard</h1>\n");
    html.push_str(&format!(
        "<p>generated_at: {} | legend: green yellow red</p>\n",
        escape_html(&generated_at)
    ));

    html.push_str("<section aria-labelledby=\"automation-health\">\n");
    html.push_str("<h2 id=\"automation-health\">Automation Health</h2>\n<dl>\n");
    html.push_str(&format!(
        "<dt>Status</dt><dd class=\"status status-{0}\">status: {0}</dd>\n",
        status
    ));
    html.push_str(&format!(
        "<dt>Consumer tag</dt><dd>{}</dd>\n",
        escape_html(consumer_tag)
    ));
    html.push_str("</dl>\n</section>\n");

    html.push_str("<section aria-labelledby=\"issues\">\n<h2 id=\"issues\">Issues</h2>\n");
    if health.issues.is_empty() {
        html.push_str("<p>none</p>\n");
    } else {
        html.push_str("<ul>\n");
        for issue in &health.issues {
            let level = automation_health_level_name(issue.level);
            html.push_str(&format!(
                "<li><span class=\"status status-{level}\">{level}</span> target={} code={} detail={}</li>\n",
                escape_html(&issue.target),
                escape_html(issue.code),
                escape_html(&issue.detail)
            ));
        }
        html.push_str("</ul>\n");
    }
    html.push_str("</section>\n");

    html.push_str(
        "<section aria-labelledby=\"last-failures\">\n<h2 id=\"last-failures\">Last Failures</h2>\n",
    );
    if health.failures.is_empty() {
        html.push_str("<p>none</p>\n");
    } else {
        html.push_str("<ul>\n");
        for failure in &health.failures {
            let detail = failure
                .latest_failure
                .as_ref()
                .map(format_automation_record)
                .unwrap_or_else(|| "latest_failure=missing".to_string());
            html.push_str(&format!(
                "<li>job={} consecutive_failures={} {}</li>\n",
                escape_html(&failure.job_name),
                failure.consecutive_failures,
                escape_html(&detail)
            ));
        }
        html.push_str("</ul>\n");
    }
    html.push_str("</section>\n");

    html.push_str(
        "<section aria-labelledby=\"metrics-summary\">\n<h2 id=\"metrics-summary\">Metrics Summary</h2>\n<dl>\n",
    );
    html.push_str(&format!(
        concat!(
            "<dt>Sources</dt><dd>{}</dd>\n",
            "<dt>Pages</dt><dd>{}</dd>\n",
            "<dt>Claims</dt><dd>{}</dd>\n",
            "<dt>Entities</dt><dd>{}</dd>\n",
            "<dt>Relations</dt><dd>{}</dd>\n",
            "<dt>Lint findings</dt><dd>total={} info={} warn={} error={}</dd>\n",
            "<dt>Gap findings</dt><dd>total={} low={} medium={} high={}</dd>\n",
            "<dt>Stale claims</dt><dd>{}</dd>\n",
            "<dt>Page status</dt><dd>{}</dd>\n",
            "<dt>Entry type</dt><dd>{}</dd>\n",
        ),
        metrics.content.sources,
        metrics.content.pages,
        metrics.content.claims,
        metrics.content.entities,
        metrics.content.relations,
        metrics.lint.total_findings,
        metrics.lint.severity.info,
        metrics.lint.severity.warn,
        metrics.lint.severity.error,
        metrics.gaps.total_findings,
        metrics.gaps.severity.low,
        metrics.gaps.severity.medium,
        metrics.gaps.severity.high,
        metrics.lifecycle.stale_claims,
        escape_html(if page_status.is_empty() {
            "none"
        } else {
            &page_status
        }),
        escape_html(if entry_type.is_empty() {
            "none"
        } else {
            &entry_type
        }),
    ));
    html.push_str("</dl>\n</section>\n");

    html.push_str("<section aria-labelledby=\"outbox\">\n<h2 id=\"outbox\">Outbox</h2>\n<dl>\n");
    html.push_str(&format!(
        "<dt>Health outbox</dt><dd>{}</dd>\n",
        escape_html(&format_outbox_stats(&health.outbox))
    ));
    html.push_str(&format!(
        concat!(
            "<dt>Metrics head id</dt><dd>{}</dd>\n",
            "<dt>Metrics total events</dt><dd>{}</dd>\n",
            "<dt>Metrics unprocessed events</dt><dd>{}</dd>\n",
            "<dt>Metrics backlog events</dt><dd>{}</dd>\n",
        ),
        escape_html(&format_optional_i64(metrics.outbox.head_id)),
        metrics.outbox.total_events,
        metrics.outbox.unprocessed_events,
        metrics.outbox.backlog_events,
    ));
    html.push_str("</dl>\n</section>\n");

    html.push_str(
        "<section aria-labelledby=\"consumer\">\n<h2 id=\"consumer\">Consumer</h2>\n<dl>\n",
    );
    html.push_str(&format!(
        "<dt>Requested tag</dt><dd>{}</dd>\n",
        escape_html(consumer_tag)
    ));
    html.push_str(&format!(
        "<dt>Metrics tag</dt><dd>{}</dd>\n",
        escape_html(metrics_consumer_tag)
    ));
    html.push_str(&format!(
        "<dt>Progress</dt><dd>{}</dd>\n",
        escape_html(&format_outbox_consumer_progress(&health.progress))
    ));
    html.push_str(&format!(
        "<dt>Metrics acked up to id</dt><dd>{}</dd>\n",
        escape_html(&format_optional_i64(metrics.outbox.acked_up_to_id))
    ));
    html.push_str("</dl>\n</section>\n");

    html.push_str("</main>\n</body>\n</html>\n");
    html
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AutomationHealthIssue, AutomationHealthLevel};
    use time::{Duration, OffsetDateTime};
    use wiki_core::{EntryStatus, EntryType};
    use wiki_storage::{
        AutomationJobFailureSummary, AutomationRunRecord, AutomationRunStatus,
        OutboxConsumerProgress, OutboxStats,
    };

    fn sample_metrics() -> WikiMetricsReport {
        let mut report =
            WikiMetricsReport::new(OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap());
        report.content.sources = 2;
        report.content.pages = 3;
        report.content.claims = 5;
        report.content.entities = 7;
        report.content.relations = 11;
        report.lint.total_findings = 13;
        report.lint.severity.warn = 17;
        report.gaps.total_findings = 19;
        report.gaps.severity.high = 23;
        report.outbox =
            wiki_core::OutboxMetrics::for_consumer(Some(29), 31, 37, "mempalace", Some(41), 43);
        report.lifecycle.stale_claims = 47;
        report.lifecycle.add_page_status(EntryStatus::Draft, 53);
        report.lifecycle.add_entry_type(EntryType::Summary, 59);
        report
    }

    fn sample_health(level: AutomationHealthLevel) -> AutomationHealthReport {
        AutomationHealthReport {
            level,
            issues: vec![AutomationHealthIssue {
                level: AutomationHealthLevel::Yellow,
                target: "lint".to_string(),
                code: "consecutive-failures",
                detail: "consecutive_failures=2".to_string(),
            }],
            outbox: OutboxStats {
                head_id: 61,
                total_events: 67,
                unprocessed_events: 71,
            },
            progress: OutboxConsumerProgress {
                consumer_tag: "mempalace".to_string(),
                acked_up_to_id: Some(73),
                acked_at: Some(OffsetDateTime::from_unix_timestamp(1_800_000_100).unwrap()),
                backlog_events: 79,
            },
            failures: vec![AutomationJobFailureSummary {
                job_name: "lint".to_string(),
                consecutive_failures: 3,
                latest_failure: Some(AutomationRunRecord {
                    id: 83,
                    job_name: "lint".to_string(),
                    started_at: OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap(),
                    finished_at: Some(
                        OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap()
                            + Duration::seconds(5),
                    ),
                    status: AutomationRunStatus::Failed,
                    duration_ms: Some(5000),
                    error_summary: Some("boom".to_string()),
                    heartbeat_at: OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap(),
                }),
            }],
        }
    }

    #[test]
    fn render_includes_required_dashboard_sections() {
        let html = render_dashboard_html(
            &sample_health(AutomationHealthLevel::Green),
            &sample_metrics(),
            "mempalace",
        );

        assert!(html.contains("Automation Health"));
        assert!(html.contains("Issues"));
        assert!(html.contains("Last Failures"));
        assert!(html.contains("Metrics Summary"));
        assert!(html.contains("Outbox"));
        assert!(html.contains("Consumer"));
    }

    #[test]
    fn render_keeps_health_status_text_stable() {
        let html = render_dashboard_html(
            &sample_health(AutomationHealthLevel::Red),
            &sample_metrics(),
            "mempalace",
        );

        assert!(html.contains("status: red"));
        assert!(html.contains("red"));
        assert!(html.contains("yellow"));
        assert!(html.contains("green"));
    }

    #[test]
    fn render_escapes_dynamic_strings() {
        let mut health = sample_health(AutomationHealthLevel::Red);
        health.issues[0].target = "lint<script>".to_string();
        health.issues[0].detail = "bad & worse".to_string();
        health.failures[0].job_name = "job\"x\"".to_string();
        health.failures[0]
            .latest_failure
            .as_mut()
            .unwrap()
            .error_summary = Some("<panic & fail>".to_string());

        let mut metrics = sample_metrics();
        metrics.outbox.consumer_tag = Some("mem<tag>".to_string());

        let html = render_dashboard_html(&health, &metrics, "consumer&tag");

        assert!(html.contains("lint&lt;script&gt;"));
        assert!(html.contains("bad &amp; worse"));
        assert!(html.contains("job&quot;x&quot;"));
        assert!(html.contains("&lt;panic &amp; fail&gt;"));
        assert!(html.contains("mem&lt;tag&gt;"));
        assert!(html.contains("consumer&amp;tag"));
        assert!(!html.contains("<script>"));
        assert!(!html.contains("<panic & fail>"));
    }

    #[test]
    fn render_empty_issues_and_failures_as_none() {
        let mut health = sample_health(AutomationHealthLevel::Green);
        health.issues.clear();
        health.failures.clear();

        let html = render_dashboard_html(&health, &sample_metrics(), "mempalace");

        assert!(html.contains("<section"));
        assert!(html.contains("Issues"));
        assert!(html.contains("Last Failures"));
        assert!(html.contains("none"));
    }
}
