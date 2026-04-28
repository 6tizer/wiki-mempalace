# Design: Scheduled Vault Reports

## Job

`AutomationJob::VaultReports` maps to CLI value `vault-reports`.

- `in_daily=true`
- `requires_network=false`
- `short_circuit=false`

The daily chain now ends with `vault-reports`, so reporting still runs after a non-short-circuit Notion sync failure.

## Output Layout

For a vault root `<vault>`:

```text
<vault>/reports/scheduled/
  latest.json
  latest.md
  <timestamp>-scheduled/
    vault-audit/
      vault-audit-<timestamp>.json
      vault-audit-<timestamp>.md
    metrics.json
    metrics.md
    dashboard.html
    automation-health.txt
    suggestions/
      <timestamp>-m12-suggest.json
      <timestamp>-m12-suggest.md
```

`latest.json` is the machine pointer. `latest.md` is the human pointer.

## Retention

`WIKI_SCHEDULED_REPORT_KEEP` controls how many timestamped scheduled report directories remain. Default: `14`. The latest pointer files are never pruned.

## Data Flow

- Vault audit uses the existing `vault_audit` scanner/writer.
- Metrics uses `collect_wiki_metrics`.
- Dashboard renders from the same metrics and automation health report.
- Automation health uses the existing health collector.
- Suggest uses the existing M12 strategy scanner.

No new report model replaces existing formats.
