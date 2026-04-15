mod classifier;
mod cli;
mod db;
mod service;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Commands, MineMode};
use service::{
    Palace, benchmark_recall_at_k, mine_path, mine_path_convos, save_benchmark_report, search,
    split_mega_file, status, taxonomy, traverse, wake_up,
};

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let palace = Palace::new(&cli.palace)?;

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
                ),
                MineMode::Convos => mine_path_convos(
                    &conn,
                    &path,
                    &palace.rules_path,
                    wing.as_deref(),
                    hall.as_deref(),
                    room.as_deref(),
                ),
            }
            .with_context(|| format!("failed to mine {}", path.display()))?;
            println!("filed {n} drawers");
        }
        Commands::Search {
            query,
            wing,
            hall,
            room,
            limit,
        } => {
            palace.init(None)?;
            let conn = palace.open()?;
            let rows = search(
                &conn,
                &query,
                wing.as_deref(),
                hall.as_deref(),
                room.as_deref(),
                limit,
            )?;
            if rows.is_empty() {
                println!("no results");
                return Ok(());
            }
            for (i, r) in rows.iter().enumerate() {
                println!(
                    "{}. #{} {} / {} / {}\n   {}\n   {}",
                    i + 1,
                    r.id,
                    r.wing,
                    r.hall,
                    r.room,
                    r.source_path,
                    r.snippet
                );
            }
        }
        Commands::Status => {
            palace.init(None)?;
            let conn = palace.open()?;
            let s = status(&conn)?;
            println!("drawers : {}", s.drawers);
            println!("wings   : {}", s.wings);
            println!("tunnels : {}", s.tunnels);
        }
        Commands::WakeUp { wing } => {
            palace.init(None)?;
            let conn = palace.open()?;
            let text = wake_up(&conn, &palace.identity_path, wing.as_deref())?;
            println!("{text}");
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
            println!("tunnel linked: {from_wing}/{from_room} -> {to_wing}/{to_room}");
        }
        Commands::Taxonomy => {
            palace.init(None)?;
            let conn = palace.open()?;
            let rows = taxonomy(&conn)?;
            if rows.is_empty() {
                println!("taxonomy is empty");
                return Ok(());
            }
            for r in rows {
                println!("{} / {} / {} => {}", r.wing, r.hall, r.room, r.count);
            }
        }
        Commands::Traverse { wing, room } => {
            palace.init(None)?;
            let conn = palace.open()?;
            let edges = traverse(&conn, &wing, &room)?;
            if edges.is_empty() {
                println!("no tunnels found from {wing}/{room}");
                return Ok(());
            }
            for e in edges {
                println!(
                    "{}:{} / {} -> {} / {}",
                    e.kind, e.from_wing, e.from_room, e.to_wing, e.to_room
                );
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
                println!("dry-run: {n} sessions would be generated");
            } else {
                println!("generated {n} split sessions");
            }
        }
        Commands::Bench {
            samples,
            top_k,
            report,
        } => {
            palace.init(None)?;
            let conn = palace.open()?;
            let b = benchmark_recall_at_k(&conn, samples, top_k)?;
            println!("samples : {}", b.total);
            println!("hits    : {}", b.hits);
            println!("recall@{}: {:.2}%", b.k, b.recall * 100.0);
            if let Some(report_path) = report {
                save_benchmark_report(&b, &report_path)?;
                println!("report  : {}", report_path.display());
            }
        }
    }
    Ok(())
}
