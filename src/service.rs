use crate::classifier::{KNOWN_HALLS, classify, default_rules, load_rules};
use crate::db;
use anyhow::Result;
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct Palace {
    pub root: PathBuf,
    pub db_path: PathBuf,
    pub identity_path: PathBuf,
    pub rules_path: PathBuf,
}

impl Palace {
    pub fn new(palace_root_raw: &str) -> Result<Self> {
        let expanded = shellexpand::tilde(palace_root_raw).to_string();
        let root = PathBuf::from(expanded);
        let db_path = root.join("palace.db");
        let identity_path = root.join("identity.txt");
        let rules_path = root.join("classifier_rules.json");
        Ok(Self {
            root,
            db_path,
            identity_path,
            rules_path,
        })
    }

    pub fn init(&self, identity: Option<&str>) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        if !self.identity_path.exists() {
            let id = identity.unwrap_or("You are my long-term coding partner. Preserve reasoning and decisions.");
            fs::write(&self.identity_path, id)?;
        }
        if !self.rules_path.exists() {
            let rules = serde_json::to_string_pretty(&default_rules())?;
            fs::write(&self.rules_path, rules)?;
        }
        let conn = self.open()?;
        db::init_schema(&conn)?;
        Ok(())
    }

    pub fn open(&self) -> Result<Connection> {
        db::open(&self.db_path)
    }
}

pub fn mine_path(
    conn: &Connection,
    target: &Path,
    rules_path: &Path,
    override_wing: Option<&str>,
    override_hall: Option<&str>,
    override_room: Option<&str>,
) -> Result<usize> {
    let rules = load_rules(rules_path);
    let mut count = 0usize;
    for entry in WalkDir::new(target).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        if !is_text_like(entry.path()) {
            continue;
        }
        let raw = match fs::read(entry.path()) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let content = match String::from_utf8(raw) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let content_trimmed = content.trim();
        if content_trimmed.is_empty() {
            continue;
        }
        let auto = classify(entry.path(), content_trimmed, rules.as_ref());
        let wing = override_wing.unwrap_or(&auto.wing);
        let hall = override_hall.unwrap_or(&auto.hall);
        let room = override_room.unwrap_or(&auto.room);
        let source_path = entry.path().to_string_lossy().to_string();
        let content_hash = sha256_hex(&source_path, content_trimmed);
        let exists = conn
            .query_row(
                "SELECT 1 FROM drawers WHERE content_hash = ?1 LIMIT 1",
                params![content_hash],
                |_| Ok(1i64),
            )
            .optional()?
            .is_some();
        if exists {
            continue;
        }
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO drawers(wing, hall, room, source_path, content, content_hash, created_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![wing, hall, room, source_path, content_trimmed, content_hash, now],
        )?;
        count += 1;
    }
    Ok(count)
}

pub fn mine_path_convos(
    conn: &Connection,
    target: &Path,
    rules_path: &Path,
    override_wing: Option<&str>,
    override_hall: Option<&str>,
    override_room: Option<&str>,
) -> Result<usize> {
    let rules = load_rules(rules_path);
    let mut count = 0usize;
    for entry in WalkDir::new(target).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() || !is_text_like(entry.path()) {
            continue;
        }
        let raw = match fs::read(entry.path()) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let text = match String::from_utf8(raw) {
            Ok(v) => v,
            Err(_) => continue,
        };
        for (idx, chunk) in convo_chunks(&text).into_iter().enumerate() {
            if chunk.trim().is_empty() {
                continue;
            }
            let auto = classify(entry.path(), &chunk, rules.as_ref());
            let wing = override_wing.unwrap_or(&auto.wing);
            let hall = override_hall.unwrap_or("hall_events");
            let room = override_room.unwrap_or(&auto.room);
            let source_path = format!("{}#chunk-{}", entry.path().to_string_lossy(), idx + 1);
            let content_hash = sha256_hex(&source_path, &chunk);
            let exists = conn
                .query_row(
                    "SELECT 1 FROM drawers WHERE content_hash = ?1 LIMIT 1",
                    params![content_hash],
                    |_| Ok(1i64),
                )
                .optional()?
                .is_some();
            if exists {
                continue;
            }
            let now = Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO drawers(wing, hall, room, source_path, content, content_hash, created_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![wing, hall, room, source_path, chunk.trim(), content_hash, now],
            )?;
            count += 1;
        }
    }
    Ok(count)
}

pub fn search(
    conn: &Connection,
    query: &str,
    wing: Option<&str>,
    hall: Option<&str>,
    room: Option<&str>,
    limit: usize,
) -> Result<Vec<SearchRow>> {
    let fts_query = build_fts_query(query);
    let sql = r#"
        SELECT d.id, d.wing, d.hall, d.room, d.source_path,
               snippet(drawers_fts, 0, '[', ']', ' ... ', 20) AS snippet,
               d.content
        FROM drawers_fts
        JOIN drawers d ON d.id = drawers_fts.rowid
        WHERE drawers_fts MATCH ?1
          AND (?2 IS NULL OR d.wing = ?2)
          AND (?3 IS NULL OR d.hall = ?3)
          AND (?4 IS NULL OR d.room = ?4)
        ORDER BY bm25(drawers_fts), d.id DESC
        LIMIT ?5
    "#;

    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query(params![fts_query, wing, hall, room, limit as i64])?;

    let mut out = Vec::new();
    while let Some(r) = rows.next()? {
        out.push(SearchRow {
            id: r.get(0)?,
            wing: r.get(1)?,
            hall: r.get(2)?,
            room: r.get(3)?,
            source_path: r.get(4)?,
            snippet: r.get(5)?,
            content: r.get(6)?,
        });
    }
    if out.is_empty() {
        let like = format!("%{}%", query);
        let mut fallback = conn.prepare(
            r#"
            SELECT id, wing, hall, room, source_path, substr(content, 1, 220)
            FROM drawers
            WHERE content LIKE ?1
              AND (?2 IS NULL OR wing = ?2)
              AND (?3 IS NULL OR hall = ?3)
              AND (?4 IS NULL OR room = ?4)
            ORDER BY id DESC
            LIMIT ?5
        "#,
        )?;
        let mut rows = fallback.query(params![like, wing, hall, room, limit as i64])?;
        while let Some(r) = rows.next()? {
            out.push(SearchRow {
                id: r.get(0)?,
                wing: r.get(1)?,
                hall: r.get(2)?,
                room: r.get(3)?,
                source_path: r.get(4)?,
                snippet: r.get(5)?,
                content: r.get(5)?,
            });
        }
    }
    rerank_rows(query, &mut out);
    Ok(out)
}

#[derive(Debug)]
pub struct SearchRow {
    pub id: i64,
    pub wing: String,
    pub hall: String,
    pub room: String,
    pub source_path: String,
    pub snippet: String,
    pub content: String,
}

pub fn status(conn: &Connection) -> Result<Status> {
    let drawers: i64 = conn.query_row("SELECT COUNT(*) FROM drawers", [], |r| r.get(0))?;
    let tunnels: i64 = conn.query_row("SELECT COUNT(*) FROM tunnels", [], |r| r.get(0))?;
    let wings: i64 = conn.query_row("SELECT COUNT(DISTINCT wing) FROM drawers", [], |r| r.get(0))?;
    Ok(Status {
        drawers,
        wings,
        tunnels,
    })
}

pub struct Status {
    pub drawers: i64,
    pub wings: i64,
    pub tunnels: i64,
}

pub fn taxonomy(conn: &Connection) -> Result<Vec<TaxonomyRow>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT wing, hall, room, COUNT(*) AS cnt
        FROM drawers
        GROUP BY wing, hall, room
        ORDER BY wing, hall, cnt DESC, room
    "#,
    )?;
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(r) = rows.next()? {
        out.push(TaxonomyRow {
            wing: r.get(0)?,
            hall: r.get(1)?,
            room: r.get(2)?,
            count: r.get(3)?,
        });
    }
    Ok(out)
}

pub struct TaxonomyRow {
    pub wing: String,
    pub hall: String,
    pub room: String,
    pub count: i64,
}

pub fn traverse(conn: &Connection, wing: &str, room: &str) -> Result<Vec<TraverseEdge>> {
    let mut out = Vec::new();
    let mut explicit = conn.prepare(
        r#"
        SELECT from_wing, from_room, to_wing, to_room, 'explicit'
        FROM tunnels
        WHERE (from_wing = ?1 AND from_room = ?2) OR (to_wing = ?1 AND to_room = ?2)
    "#,
    )?;
    let mut rows = explicit.query(params![wing, room])?;
    while let Some(r) = rows.next()? {
        out.push(TraverseEdge {
            from_wing: r.get(0)?,
            from_room: r.get(1)?,
            to_wing: r.get(2)?,
            to_room: r.get(3)?,
            kind: r.get(4)?,
        });
    }

    let mut implicit = conn.prepare(
        r#"
        SELECT DISTINCT ?1, ?2, wing, room, 'implicit'
        FROM drawers
        WHERE room = ?2 AND wing != ?1
    "#,
    )?;
    let mut rows = implicit.query(params![wing, room])?;
    while let Some(r) = rows.next()? {
        out.push(TraverseEdge {
            from_wing: r.get(0)?,
            from_room: r.get(1)?,
            to_wing: r.get(2)?,
            to_room: r.get(3)?,
            kind: r.get(4)?,
        });
    }
    Ok(out)
}

pub struct TraverseEdge {
    pub from_wing: String,
    pub from_room: String,
    pub to_wing: String,
    pub to_room: String,
    pub kind: String,
}

pub fn benchmark_recall_at_k(conn: &Connection, samples: usize, k: usize) -> Result<BenchmarkResult> {
    let mut stmt = conn.prepare(
        "SELECT id, content FROM drawers ORDER BY id DESC LIMIT ?1"
    )?;
    let mut rows = stmt.query(params![samples as i64])?;
    let mut total = 0usize;
    let mut hits = 0usize;
    while let Some(r) = rows.next()? {
        let id: i64 = r.get(0)?;
        let content: String = r.get(1)?;
        let query = content.split_whitespace().take(8).collect::<Vec<_>>().join(" ");
        if query.trim().is_empty() {
            continue;
        }
        total += 1;
        let result = search(conn, &query, None, None, None, k)?;
        if result.iter().any(|row| row.id == id) {
            hits += 1;
        }
    }
    let recall = if total == 0 { 0.0 } else { hits as f64 / total as f64 };
    Ok(BenchmarkResult { total, hits, recall, k })
}

pub struct BenchmarkResult {
    pub total: usize,
    pub hits: usize,
    pub recall: f64,
    pub k: usize,
}

pub fn save_benchmark_report(result: &BenchmarkResult, report_path: &Path) -> Result<()> {
    let parent = report_path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    if report_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .eq_ignore_ascii_case("json")
    {
        let payload = BenchmarkReportJson {
            total: result.total,
            hits: result.hits,
            recall: result.recall,
            top_k: result.k,
            generated_at: Utc::now().to_rfc3339(),
        };
        fs::write(report_path, serde_json::to_string_pretty(&payload)?)?;
    } else {
        let body = format!(
            "# Benchmark Report\n\n- generated_at: {}\n- samples: {}\n- hits: {}\n- recall@{}: {:.2}%\n",
            Utc::now().to_rfc3339(),
            result.total,
            result.hits,
            result.k,
            result.recall * 100.0
        );
        fs::write(report_path, body)?;
    }
    Ok(())
}

pub fn wake_up(conn: &Connection, identity_path: &Path, wing: Option<&str>) -> Result<String> {
    let identity = fs::read_to_string(identity_path).unwrap_or_default();
    let mut out = String::new();
    out.push_str("# L0 Identity\n");
    out.push_str(identity.trim());
    out.push_str("\n\n# L1 Critical Facts\n");
    for hall in KNOWN_HALLS {
        let mut sql = String::from("SELECT room, substr(content, 1, 180) FROM drawers WHERE hall = ?1");
        if wing.is_some() {
            sql.push_str(" AND wing = ?2 ");
        }
        sql.push_str(" ORDER BY id DESC LIMIT 2");
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = if let Some(w) = wing {
            stmt.query(params![hall, w])?
        } else {
            stmt.query(params![hall])?
        };
        while let Some(r) = rows.next()? {
            let room_name: String = r.get(0)?;
            let text: String = r.get(1)?;
            out.push_str(&format!("- {hall}::{room_name}: {}\n", one_line(&text)));
        }
    }
    if out.trim().is_empty() {
        anyhow::bail!("no wake-up context generated");
    }
    Ok(out)
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn rerank_rows(query: &str, rows: &mut Vec<SearchRow>) {
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|s| s.to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    rows.sort_by(|a, b| {
        let sa = simple_relevance_score(&a.content, &terms, a.id);
        let sb = simple_relevance_score(&b.content, &terms, b.id);
        sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn simple_relevance_score(content: &str, terms: &[String], id: i64) -> f64 {
    let lc = content.to_ascii_lowercase();
    let mut score = 0.0;
    for t in terms {
        let c = lc.matches(t).count() as f64;
        score += c * 2.0;
        if lc.contains(&format!("{t}:")) {
            score += 1.0;
        }
    }
    let trigram = trigram_similarity(&lc, &terms.join(" "));
    score + trigram * 3.0 + (id as f64 * 0.00001)
}

fn build_fts_query(query: &str) -> String {
    let tokens: Vec<String> = query
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase())
        .collect();
    if tokens.is_empty() {
        "memory".to_string()
    } else {
        tokens.join(" ")
    }
}

fn convo_chunks(text: &str) -> Vec<String> {
    let normalized = text.replace("\r\n", "\n");
    let markers = ["\nUser:", "\nAssistant:", "\n### Session", "\n---"];
    let mut chunks = vec![normalized];
    for m in markers {
        let mut next = Vec::new();
        for c in chunks {
            let parts: Vec<String> = c.split(m).map(|s| s.trim().to_string()).collect();
            if parts.len() > 1 {
                for p in parts {
                    if !p.is_empty() {
                        next.push(p);
                    }
                }
            } else {
                next.push(c);
            }
        }
        chunks = next;
    }
    let mut compact = Vec::new();
    let mut buf = String::new();
    for c in chunks {
        if c.lines().count() >= 2 {
            if !buf.is_empty() {
                compact.push(buf.clone());
                buf.clear();
            }
            compact.push(c);
        } else {
            if !buf.is_empty() {
                buf.push('\n');
            }
            buf.push_str(&c);
        }
    }
    if !buf.trim().is_empty() {
        compact.push(buf);
    }
    compact
}

fn trigram_similarity(a: &str, b: &str) -> f64 {
    let sa = trigrams(a);
    let sb = trigrams(b);
    if sa.is_empty() || sb.is_empty() {
        return 0.0;
    }
    let inter = sa.intersection(&sb).count() as f64;
    let union = sa.union(&sb).count() as f64;
    if union == 0.0 { 0.0 } else { inter / union }
}

fn trigrams(s: &str) -> std::collections::HashSet<String> {
    let clean = s
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let chars: Vec<char> = clean.chars().collect();
    let mut out = std::collections::HashSet::new();
    if chars.len() < 3 {
        if !clean.is_empty() {
            out.insert(clean);
        }
        return out;
    }
    for i in 0..=(chars.len() - 3) {
        out.insert(chars[i..i + 3].iter().collect());
    }
    out
}

#[derive(Serialize)]
struct BenchmarkReportJson {
    total: usize,
    hits: usize,
    recall: f64,
    top_k: usize,
    generated_at: String,
}

fn sha256_hex(path: &str, content: &str) -> String {
    let mut h = Sha256::new();
    h.update(path.as_bytes());
    h.update(b"\n");
    h.update(content.as_bytes());
    format!("{:x}", h.finalize())
}

fn is_text_like(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
        return true;
    };
    let ext = ext.to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "txt"
            | "md"
            | "markdown"
            | "rs"
            | "toml"
            | "json"
            | "yaml"
            | "yml"
            | "py"
            | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "java"
            | "go"
            | "c"
            | "cpp"
            | "h"
            | "hpp"
            | "log"
            | "csv"
    )
}

pub fn split_mega_file(path: &Path, marker: &str, min_lines: usize, dry_run: bool) -> Result<usize> {
    let text = fs::read_to_string(path)?;
    let lines: Vec<&str> = text.lines().collect();
    let mut chunks: Vec<Vec<&str>> = Vec::new();
    let mut cur: Vec<&str> = Vec::new();

    for line in lines {
        if line.starts_with(marker) && !cur.is_empty() {
            chunks.push(cur);
            cur = vec![line];
        } else {
            cur.push(line);
        }
    }
    if !cur.is_empty() {
        chunks.push(cur);
    }

    let mut written = 0usize;
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("session");
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    for (idx, chunk) in chunks.iter().enumerate() {
        if chunk.len() < min_lines {
            continue;
        }
        written += 1;
        if dry_run {
            continue;
        }
        let out_name = format!("{stem}.session-{:03}.txt", idx + 1);
        let out_path = parent.join(out_name);
        fs::write(out_path, chunk.join("\n"))?;
    }
    Ok(written)
}
