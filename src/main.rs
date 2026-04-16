mod classifier;
mod cli;
mod db;
mod llm;
mod mcp;
mod service;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{BenchMode, Cli, Commands, McpTransport, MineMode, OutputFormat};
use serde_json::json;
use service::{
    Palace, banner, benchmark_run, drawer_content, extract_to_kg, kg_add, kg_conflicts,
    kg_invalidate, kg_query, kg_stats, kg_timeline, load_config, mine_path, mine_path_convos,
    principles_report, reflect_answer, save_benchmark_report, search_with_options, split_mega_file,
    status, taxonomy, traverse, wake_up,
};

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = match Cli::try_parse() {
        Ok(v) => v,
        Err(e) => {
            println!("{}", banner());
            e.print()?;
            return Ok(());
        }
    };
    let palace = Palace::new(&cli.palace)?;
    let config = load_config(&palace.config_path);
    if !cli.quiet && !matches!(cli.command, Commands::Mcp { .. }) {
        println!("{}", banner());
    }

    match cli.command {
        Commands::Init { identity } => {
            palace.init(identity.as_deref())?;
            println!("palace initialized at {}", palace.root.display());
        }
        Commands::Mine {
            path,
            mode,
            wing,
            hall,
            room,
            bank,
        } => {
            palace.init(None)?;
            let conn = palace.open()?;
            let n = match mode {
                MineMode::Projects => mine_path(
                    &conn,
                    &path,
                    &palace.rules_path,
                    wing.as_deref(),
                    hall.as_deref(),
                    room.as_deref(),
                    Some(bank.as_str()),
                ),
                MineMode::Convos => mine_path_convos(
                    &conn,
                    &path,
                    &palace.rules_path,
                    wing.as_deref(),
                    hall.as_deref(),
                    room.as_deref(),
                    Some(bank.as_str()),
                ),
            }
            .with_context(|| format!("failed to mine {}", path.display()))?;
            print_out(
                cli.output,
                json!({"filed_drawers": n}),
                &format!("filed {n} drawers"),
            );
        }
        Commands::Search {
            query,
            wing,
            hall,
            room,
            bank,
            limit,
        } => {
            palace.init(None)?;
            let conn = palace.open()?;
            let rows = search_with_options(
                &conn,
                &query,
                wing.as_deref(),
                hall.as_deref(),
                room.as_deref(),
                bank.as_deref(),
                limit,
                &config.retrieval,
                cli.output == OutputFormat::Json,
            )?;
            if rows.is_empty() {
                print_out(cli.output, json!({"results": []}), "no results");
                return Ok(());
            }
            if cli.output == OutputFormat::Json {
                print_out(
                    cli.output,
                    json!({"results": rows.iter().map(|r| json!({
                        "id": r.id,
                        "wing": r.wing,
                        "hall": r.hall,
                        "room": r.room,
                        "bank_id": r.bank_id,
                        "source_path": r.source_path,
                        "snippet": r.snippet,
                        "score": r.score,
                        "explain": r.explain
                    })).collect::<Vec<_>>()}),
                    "",
                );
            } else {
                for (i, r) in rows.iter().enumerate() {
                    println!(
                        "{}. #{} {} / {} / {} [bank={}]\n   {}\n   {}\n   score={:.4}",
                        i + 1,
                        r.id,
                        r.wing,
                        r.hall,
                        r.room,
                        r.bank_id,
                        r.source_path,
                        r.snippet,
                        r.score
                    );
                }
            }
        }
        Commands::Status => {
            palace.init(None)?;
            let conn = palace.open()?;
            let s = status(&conn)?;
            print_out(
                cli.output,
                json!({"drawers": s.drawers, "wings": s.wings, "tunnels": s.tunnels, "kg_facts": s.kg_facts}),
                &format!(
                    "drawers : {}\nwings   : {}\ntunnels : {}\nkg_facts: {}",
                    s.drawers, s.wings, s.tunnels, s.kg_facts
                ),
            );
        }
        Commands::WakeUp { wing, bank } => {
            palace.init(None)?;
            let conn = palace.open()?;
            let text = wake_up(
                &conn,
                &palace.identity_path,
                wing.as_deref(),
                bank.as_deref(),
            )?;
            print_out(cli.output, json!({"text": text}), &text);
        }
        Commands::Link {
            from_wing,
            from_room,
            to_wing,
            to_room,
        } => {
            palace.init(None)?;
            let conn = palace.open()?;
            let now = chrono::Utc::now().to_rfc3339();
            db::insert_tunnel(&conn, &from_wing, &from_room, &to_wing, &to_room, &now)?;
            print_out(
                cli.output,
                json!({"linked": true, "from_wing": from_wing, "from_room": from_room, "to_wing": to_wing, "to_room": to_room}),
                &format!("tunnel linked: {from_wing}/{from_room} -> {to_wing}/{to_room}"),
            );
        }
        Commands::Taxonomy { bank } => {
            palace.init(None)?;
            let conn = palace.open()?;
            let rows = taxonomy(&conn, bank.as_deref())?;
            if rows.is_empty() {
                print_out(cli.output, json!({"taxonomy": []}), "taxonomy is empty");
                return Ok(());
            }
            if cli.output == OutputFormat::Json {
                print_out(
                    cli.output,
                    json!({"taxonomy": rows.into_iter().map(|r| json!({"wing":r.wing,"hall":r.hall,"room":r.room,"count":r.count})).collect::<Vec<_>>()}),
                    "",
                );
            } else {
                for r in rows {
                    println!("{} / {} / {} => {}", r.wing, r.hall, r.room, r.count);
                }
            }
        }
        Commands::Traverse { wing, room, bank } => {
            palace.init(None)?;
            let conn = palace.open()?;
            let edges = traverse(&conn, &wing, &room, bank.as_deref())?;
            if edges.is_empty() {
                print_out(
                    cli.output,
                    json!({"traverse":[],"wing":wing,"room":room}),
                    &format!("no tunnels found from {wing}/{room}"),
                );
                return Ok(());
            }
            if cli.output == OutputFormat::Json {
                print_out(
                    cli.output,
                    json!({"traverse":edges.into_iter().map(|e| json!({"kind":e.kind,"from_wing":e.from_wing,"from_room":e.from_room,"to_wing":e.to_wing,"to_room":e.to_room})).collect::<Vec<_>>()}),
                    "",
                );
            } else {
                for e in edges {
                    println!(
                        "{}:{} / {} -> {} / {}",
                        e.kind, e.from_wing, e.from_room, e.to_wing, e.to_room
                    );
                }
            }
        }
        Commands::Split {
            path,
            marker,
            min_lines,
            dry_run,
        } => {
            let n = split_mega_file(&path, &marker, min_lines, dry_run)
                .with_context(|| format!("failed to split {}", path.display()))?;
            if dry_run {
                print_out(
                    cli.output,
                    json!({"dry_run":true,"sessions":n}),
                    &format!("dry-run: {n} sessions would be generated"),
                );
            } else {
                print_out(
                    cli.output,
                    json!({"dry_run":false,"sessions":n}),
                    &format!("generated {n} split sessions"),
                );
            }
        }
        Commands::Bench {
            samples,
            top_k,
            mode,
            report,
        } => {
            palace.init(None)?;
            let conn = palace.open()?;
            let mode_s = match mode {
                BenchMode::Random => "random",
                BenchMode::Fixed => "fixed",
            };
            let b = benchmark_run(&conn, samples, top_k, mode_s)?;
            print_out(
                cli.output,
                json!({"mode": b.mode, "samples": b.total, "hits": b.hits, "recall_at_k": b.recall, "k": b.k, "latency_ms": b.latency_ms, "throughput_per_sec": b.throughput_per_sec}),
                &format!(
                    "mode    : {}\nsamples : {}\nhits    : {}\nrecall@{}: {:.2}%\nlatency : {} ms\nthroughput: {:.2}/s",
                    b.mode,
                    b.total,
                    b.hits,
                    b.k,
                    b.recall * 100.0,
                    b.latency_ms,
                    b.throughput_per_sec
                ),
            );
            if let Some(report_path) = report {
                save_benchmark_report(&b, &report_path)?;
                println!("report  : {}", report_path.display());
            }
        }
        Commands::Banner => {
            // Banner is now auto-shown on every CLI startup.
        }
        Commands::Principles => {
            palace.init(None)?;
            let conn = palace.open()?;
            let text = principles_report(&conn)?;
            print_out(cli.output, json!({"principles": text}), &text);
        }
        Commands::KgAdd {
            subject,
            predicate,
            object,
            valid_from,
            source_drawer_id,
        } => {
            palace.init(None)?;
            let conn = palace.open()?;
            kg_add(
                &conn,
                &subject,
                &predicate,
                &object,
                valid_from.as_deref(),
                source_drawer_id,
            )?;
            println!("kg fact added: {} {} {}", subject, predicate, object);
        }
        Commands::KgQuery { subject, as_of } => {
            palace.init(None)?;
            let conn = palace.open()?;
            let rows = kg_query(&conn, &subject, as_of.as_deref())?;
            if rows.is_empty() {
                print_out(
                    cli.output,
                    json!({"facts":[],"subject":subject}),
                    &format!("no active facts for subject={subject}"),
                );
                return Ok(());
            }
            if cli.output == OutputFormat::Json {
                print_out(
                    cli.output,
                    json!({"facts": rows.iter().map(|r| json!({"id":r.id,"subject":r.subject,"predicate":r.predicate,"object":r.object,"valid_from":r.valid_from,"valid_to":r.valid_to,"source_drawer_id":r.source_drawer_id})).collect::<Vec<_>>()}),
                    "",
                );
            } else {
                for row in rows {
                    println!(
                        "#{} {} --{}--> {} [{} .. {}] src={}",
                        row.id,
                        row.subject,
                        row.predicate,
                        row.object,
                        row.valid_from,
                        row.valid_to.unwrap_or_else(|| "open".to_string()),
                        row.source_drawer_id
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "-".to_string())
                    );
                }
            }
        }
        Commands::KgTimeline { subject } => {
            palace.init(None)?;
            let conn = palace.open()?;
            let rows = kg_timeline(&conn, &subject)?;
            if cli.output == OutputFormat::Json {
                print_out(
                    cli.output,
                    json!({"timeline": rows.iter().map(|r| json!({"id":r.id,"subject":r.subject,"predicate":r.predicate,"object":r.object,"valid_from":r.valid_from,"valid_to":r.valid_to,"source_drawer_id":r.source_drawer_id})).collect::<Vec<_>>()}),
                    "",
                );
            } else {
                for r in rows {
                    println!(
                        "{} {} -> {} [{}..{}]",
                        r.subject,
                        r.predicate,
                        r.object,
                        r.valid_from,
                        r.valid_to.unwrap_or_else(|| "open".to_string())
                    );
                }
            }
        }
        Commands::KgStats => {
            palace.init(None)?;
            let conn = palace.open()?;
            let s = kg_stats(&conn)?;
            print_out(
                cli.output,
                json!({"facts":s.facts,"subjects":s.subjects,"predicates":s.predicates,"active_facts":s.active_facts}),
                &format!(
                    "facts: {}\nsubjects: {}\npredicates: {}\nactive_facts: {}",
                    s.facts, s.subjects, s.predicates, s.active_facts
                ),
            );
        }
        Commands::KgConflicts => {
            palace.init(None)?;
            let conn = palace.open()?;
            let conflicts = kg_conflicts(&conn)?;
            if cli.output == OutputFormat::Json {
                print_out(
                    cli.output,
                    json!({"conflicts": conflicts.iter().map(|c| json!({"subject":c.subject,"predicate":c.predicate,"objects":c.objects})).collect::<Vec<_>>()}),
                    "",
                );
            } else if conflicts.is_empty() {
                println!("no conflicts");
            } else {
                for c in conflicts {
                    println!("conflict: {} {} => {:?}", c.subject, c.predicate, c.objects);
                }
            }
        }
        Commands::KgInvalidate {
            subject,
            predicate,
            object,
            ended,
        } => {
            palace.init(None)?;
            let conn = palace.open()?;
            let changed = kg_invalidate(&conn, &subject, &predicate, &object, ended.as_deref())?;
            println!("invalidated {changed} facts");
        }
        Commands::Reflect {
            query,
            search_limit,
            bank,
        } => {
            palace.init(None)?;
            let conn = palace.open()?;
            let text = reflect_answer(
                &conn,
                &config.llm,
                &config.retrieval,
                &query,
                bank.as_deref(),
                search_limit,
            )?;
            print_out(cli.output, json!({"text": text}), &text);
        }
        Commands::Extract { text, drawer_id } => {
            palace.init(None)?;
            let conn = palace.open()?;
            let body = match (&text, drawer_id) {
                (Some(t), None) => t.clone(),
                (None, Some(id)) => drawer_content(&conn, id)?
                    .ok_or_else(|| anyhow::anyhow!("drawer id not found"))?,
                (None, None) => anyhow::bail!("provide --text or --drawer-id"),
                (Some(_), Some(_)) => anyhow::bail!("use only one of --text or --drawer-id"),
            };
            let n = extract_to_kg(&conn, &config.llm, &body)?;
            print_out(
                cli.output,
                json!({"kg_facts_added": n}),
                &format!("added {n} kg facts from extract"),
            );
        }
        Commands::Mcp {
            once,
            transport,
            quiet,
        } => match transport {
            McpTransport::Stdio => {
                mcp::run_stdio(&palace, once, quiet || config.mcp.quiet_default, &config)?;
            }
        },
    }
    Ok(())
}

fn print_out(format: OutputFormat, json_value: serde_json::Value, text_value: &str) {
    match format {
        OutputFormat::Json => println!("{json_value}"),
        OutputFormat::Text => {
            if !text_value.is_empty() {
                println!("{text_value}");
            }
        }
    };
}
