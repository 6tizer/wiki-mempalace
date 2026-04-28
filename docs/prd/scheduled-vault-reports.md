# PRD: Scheduled Vault Reports

## Status

Implemented in PR #67.

## Goal

Generate the recurring human and machine reports needed to inspect the active Vault without manually running five separate commands.

## Scope

- Add an automation job for scheduled Vault reports.
- Generate vault-audit, metrics, dashboard, automation health, and M12 suggest reports.
- Store each run under a timestamped directory.
- Maintain stable latest pointers.
- Prune old scheduled report runs with a configurable keep count.

## Out of Scope

- Production apply or mutation of wiki content.
- External notification delivery.
- Changing existing report formats.
- M12 executor actions.

## Success Criteria

- `wiki-cli automation run vault-reports` writes a full report bundle under `<wiki-dir>/reports/scheduled/`.
- `automation run-daily --dry-run` includes `vault-reports` after the data refresh jobs.
- `latest.json` and `latest.md` point to the newest generated bundle.
- Old timestamped report directories are pruned by retention policy.
