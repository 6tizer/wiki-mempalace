use crate::cli::{
    AiProfileCmd, AutomationCmd, Cli, Cmd, GovernanceCmd, OrphanGovernanceCmd,
    ResearchSynthesisCmd, WebSearchCmd,
};
use crate::cli_utils::resolve_wiki_relative_path;
use crate::{
    acquire_cli_writer_lease, automation_run_daily_jobs, llm, orphan_governance,
    print_automation_jobs, run_automation_plan, run_verify_row_state, vault_audit, vault_backfill,
    web_search,
};
use std::collections::BTreeSet;
use time::OffsetDateTime;
use wiki_core::{
    parse_semantic_patch_proposals_json, GovernanceDuplicateGroup, GovernanceScanReport,
};
use wiki_kernel::{
    build_evidence_fixer_plan, discover_synthesis_candidates, duplicate_web_verification_key,
    EvidenceFixerPlanOptions, SynthesisDiscoveryOptions,
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

    if let Cmd::VerifyRowState { json } = &cli.cmd {
        run_verify_row_state(&cli.db, *json)?;
        return Ok(true);
    }

    if let Cmd::AiProfile {
        cmd: AiProfileCmd::Smoke { profile, prompt },
    } = &cli.cmd
    {
        let cfg = llm::load_llm_profile_config(&cli.llm_config, Some(profile))?;
        let out = llm::smoke_chat_completion(&cfg, prompt)?;
        println!("{out}");
        return Ok(true);
    }

    if let Cmd::WebSearch {
        cmd:
            WebSearchCmd::Smoke {
                providers,
                query,
                json,
            },
    } = &cli.cmd
    {
        let app = llm::load_app_config(&cli.llm_config)?;
        let run = web_search::run_search(&app, providers, query)?;
        if *json {
            println!("{}", serde_json::to_string_pretty(&run)?);
        } else {
            println!(
                "web_search providers_succeeded={} distinct_domains={} cross_verified={} evidence={}",
                run.providers_succeeded.len(),
                run.distinct_domains,
                run.cross_verified,
                run.evidence.len()
            );
            for item in run.evidence.iter().take(10) {
                println!(
                    "{}\t{}\t{}\t{}",
                    item.provider, item.domain, item.title, item.url
                );
            }
        }
        return Ok(true);
    }

    if let Cmd::Governance {
        cmd:
            GovernanceCmd::FixerPlan {
                scan,
                json,
                report_dir,
                semantic_patches,
                allow_web_search,
                allow_private_web_search,
                internal_only,
                max_web_checks,
            },
    } = &cli.cmd
    {
        let raw_scan = std::fs::read_to_string(scan)?;
        let scan_report: GovernanceScanReport = serde_json::from_str(&raw_scan)?;
        let semantic_patch_proposals = match semantic_patches {
            Some(path) => parse_semantic_patch_proposals_json(&std::fs::read_to_string(path)?)?,
            None => Vec::new(),
        };
        let (web_available, web_cross_verified_keys) = maybe_cross_verify_duplicates(
            cli,
            &scan_report,
            *allow_web_search,
            *allow_private_web_search,
            *internal_only,
            *max_web_checks,
        );
        let now = OffsetDateTime::now_utc();
        let plan = build_evidence_fixer_plan(
            &scan_report,
            EvidenceFixerPlanOptions {
                generated_at: now,
                plan_id: crate::governance::fixer_plan_prefix(now),
                web_available,
                web_cross_verified_keys,
                semantic_patch_proposals,
            },
        );
        let files = report_dir
            .clone()
            .map(|dir| resolve_wiki_relative_path(cli.wiki_dir.as_deref(), dir))
            .map(|dir| crate::governance::write_fixer_plan_files(&plan, &dir))
            .transpose()?;
        if *json {
            println!("{}", serde_json::to_string_pretty(&plan)?);
        } else {
            print!("{}", crate::governance::render_fixer_plan_text(&plan));
            if let Some(files) = files {
                println!("json_report_file={}", files.json_path.display());
                println!("markdown_report_file={}", files.markdown_path.display());
            }
        }
        return Ok(true);
    }

    if let Cmd::ResearchSynthesis {
        cmd:
            ResearchSynthesisCmd::Discover {
                scan: Some(scan),
                json,
                report_dir,
                max_single_double,
                max_triple,
                max_quad,
            },
    } = &cli.cmd
    {
        let raw_scan = std::fs::read_to_string(scan)?;
        let scan_report: GovernanceScanReport = serde_json::from_str(&raw_scan)?;
        let now = OffsetDateTime::now_utc();
        let report = discover_synthesis_candidates(
            &scan_report,
            SynthesisDiscoveryOptions {
                generated_at: now,
                report_id: crate::governance::synthesis_discovery_report_prefix(now),
                max_single_double: *max_single_double,
                max_triple: *max_triple,
                max_quad: *max_quad,
            },
        );
        let files = report_dir
            .clone()
            .map(|dir| resolve_wiki_relative_path(cli.wiki_dir.as_deref(), dir))
            .map(|dir| crate::governance::write_synthesis_discovery_files(&report, &dir))
            .transpose()?;
        if *json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            print!(
                "{}",
                crate::governance::render_synthesis_discovery_text(&report)
            );
            if let Some(files) = files {
                println!("json_report_file={}", files.json_path.display());
                println!("markdown_report_file={}", files.markdown_path.display());
            }
        }
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

fn maybe_cross_verify_duplicates(
    cli: &Cli,
    scan_report: &GovernanceScanReport,
    allow_web_search: bool,
    allow_private_web_search: bool,
    internal_only: bool,
    max_web_checks: usize,
) -> (bool, BTreeSet<String>) {
    if internal_only || !allow_web_search || max_web_checks == 0 {
        return (false, BTreeSet::new());
    }
    if scan_report
        .viewer_scope
        .as_deref()
        .is_some_and(|scope| scope.trim_start().starts_with("private:"))
        && !allow_private_web_search
    {
        eprintln!("fixer-plan web verification skipped: private viewer_scope requires --allow-private-web-search");
        return (false, BTreeSet::new());
    }

    let app = match llm::load_app_config(&cli.llm_config) {
        Ok(app) => app,
        Err(err) => {
            eprintln!("fixer-plan web verification skipped: {}", err);
            return (false, BTreeSet::new());
        }
    };

    let mut web_available = false;
    let mut verified = BTreeSet::new();
    for group in scan_report
        .duplicates
        .iter()
        .filter(|group| group.confidence != "exact")
        .take(max_web_checks)
    {
        let query = duplicate_verification_query(group);
        match web_search::run_search(&app, &[], &query) {
            Ok(run) => {
                if run.providers_succeeded.len() >= 2 {
                    web_available = true;
                }
                if run.cross_verified {
                    verified.insert(duplicate_web_verification_key(group));
                } else {
                    eprintln!(
                        "fixer-plan web verification not met: key={} providers={} domains={}",
                        duplicate_web_verification_key(group),
                        run.providers_succeeded.len(),
                        run.distinct_domains
                    );
                }
            }
            Err(err) => {
                eprintln!(
                    "fixer-plan web verification skipped for key={}: {}",
                    duplicate_web_verification_key(group),
                    err
                );
            }
        }
    }
    (web_available, verified)
}

fn duplicate_verification_query(group: &GovernanceDuplicateGroup) -> String {
    let labels = group
        .members
        .iter()
        .filter_map(|member| member.label.as_deref())
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .collect::<Vec<_>>();
    if labels.is_empty() {
        format!("{} {}", group.kind, group.key)
    } else {
        format!(
            "verify whether these refer to the same object: {}",
            labels.join(" | ")
        )
    }
}
