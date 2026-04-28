use crate::{
    acquire_cli_writer_lease, automation_run_daily_jobs, llm, orphan_governance,
    print_automation_jobs, run_automation_plan, vault_audit, vault_backfill, AutomationCmd, Cli,
    Cmd, OrphanGovernanceCmd,
};

pub(crate) fn maybe_run_without_engine(cli: &Cli) -> Result<bool, Box<dyn std::error::Error>> {
    if matches!(
        &cli.cmd,
        Cmd::Automation {
            cmd: AutomationCmd::ListJobs
        }
    ) {
        let mut stdout = std::io::stdout().lock();
        print_automation_jobs(&mut stdout)?;
        return Ok(true);
    }

    if matches!(
        &cli.cmd,
        Cmd::Automation {
            cmd: AutomationCmd::RunDaily { dry_run: true }
        }
    ) {
        let jobs = automation_run_daily_jobs();
        let mut stdout = std::io::stdout().lock();
        run_automation_plan(&jobs, true, &mut stdout, |_| Ok(()))?;
        return Ok(true);
    }

    if let Cmd::VaultAudit { vault, report_dir } = &cli.cmd {
        let report = vault_audit::scan_vault(vault)?;
        let files = match report_dir {
            Some(dir) => vault_audit::write_json_and_markdown(&report, dir)
                .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?,
            None => vault_audit::write_json_and_markdown_in_vault_reports(&report)
                .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?,
        };
        println!(
            "vault_audit sources={} pages={} ready_sources={} ready_pages={}",
            report.sources.total,
            report.pages.total,
            report.readiness.ready_sources,
            report.readiness.ready_pages,
        );
        println!("json_report_file={}", files.json_path.display());
        println!("markdown_report_file={}", files.markdown_path.display());
        return Ok(true);
    }

    if let Cmd::OrphanGovernance { command } = &cli.cmd {
        match command {
            OrphanGovernanceCmd::Plan {
                audit_report,
                report_dir,
            } => {
                let llm_config_path = cli.llm_config.clone();
                let (plan, files) = orphan_governance::run_plan_with_llm(
                    audit_report,
                    report_dir.clone(),
                    cli.wiki_dir.as_deref(),
                    move |system, user, max_tokens| {
                        let cfg = llm::load_llm_config(&llm_config_path).map_err(
                            |e| -> Box<dyn std::error::Error + Send + Sync> {
                                e.to_string().into()
                            },
                        )?;
                        llm::complete_chat(&cfg, system, user, max_tokens).map_err(
                            |e| -> Box<dyn std::error::Error + Send + Sync> {
                                e.to_string().into()
                            },
                        )
                    },
                )
                .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
                println!(
                    "orphan_governance_plan actions={} executable_actions={}",
                    plan.actions.len(),
                    plan.actions
                        .iter()
                        .filter(|action| matches!(
                            action.action_type.as_str(),
                            "insert_page_status"
                                | "insert_source_compiled_to_wiki"
                                | "delete_cleanup_path"
                        ))
                        .count(),
                );
                println!("json_report_file={}", files.json_path.display());
                println!("markdown_report_file={}", files.markdown_path.display());
            }
            OrphanGovernanceCmd::Apply { plan, apply } => {
                let wiki_dir = cli.wiki_dir.as_deref().ok_or_else(|| {
                    "--wiki-dir is required for orphan-governance apply".to_string()
                })?;
                let report = orphan_governance::run_apply(plan, wiki_dir, *apply)
                    .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
                println!(
                    "orphan_governance_apply mode={} actions_seen={} executable_actions={} page_status_insertions_planned={} source_compiled_to_wiki_insertions_planned={} cleanup_deletions_planned={} page_status_insertions_applied={} source_compiled_to_wiki_insertions_applied={} cleanup_deletions_applied={}",
                    report.mode,
                    report.actions_seen,
                    report.executable_actions,
                    report.page_status_insertions_planned,
                    report.source_compiled_to_wiki_insertions_planned,
                    report.cleanup_deletions_planned,
                    report.page_status_insertions_applied,
                    report.source_compiled_to_wiki_insertions_applied,
                    report.cleanup_deletions_applied,
                );
            }
        }
        return Ok(true);
    }

    if let Cmd::VaultBackfill {
        vault,
        scope,
        dry_run,
        apply,
        limit,
        report_dir,
    } = &cli.cmd
    {
        if *dry_run && *apply {
            return Err("--dry-run and --apply cannot be used together".into());
        }
        let mode = if *apply {
            vault_backfill::BackfillMode::Apply
        } else {
            vault_backfill::BackfillMode::DryRun
        };
        let _writer_lease = if *apply {
            Some(acquire_cli_writer_lease(&cli.db, "vault-backfill")?)
        } else {
            None
        };
        let report_dir = report_dir.clone().unwrap_or_else(|| vault.join("reports"));
        let report = vault_backfill::backfill_vault(vault_backfill::VaultBackfillOptions {
            vault_path: vault.clone(),
            db_path: cli.db.clone(),
            scope: vault_backfill::parse_scope(scope)?,
            mode,
            limit: *limit,
            report_dir: report_dir.clone(),
        })?;
        println!(
            "vault_backfill mode={} sources_imported={} sources_updated={} pages_imported={} pages_updated={} page_written_events={} skipped={}",
            report.mode,
            report.sources_imported,
            report.sources_updated,
            report.pages_imported,
            report.pages_updated,
            report.page_written_events,
            report.skipped.len(),
        );
        println!(
            "json_report_file={}",
            report_dir.join("vault-backfill-report.json").display()
        );
        println!(
            "markdown_report_file={}",
            report_dir.join("vault-backfill-report.md").display()
        );
        return Ok(true);
    }

    Ok(false)
}
