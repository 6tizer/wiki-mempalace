use crate::classifier::{classify, default_rules, load_rules, KNOWN_HALLS};
use crate::db;
use anyhow::{Context, Result};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use walkdir::WalkDir;

pub struct Palace {
    pub root: PathBuf,
    pub db_path: PathBuf,
    pub identity_path: PathBuf,
    pub rules_path: PathBuf,
    pub config_path: PathBuf,
}

impl Palace {
    pub fn new(palace_root_raw: &str) -> Result<Self> {
        let expanded = shellexpand::tilde(palace_root_raw).to_string();
        let root = PathBuf::from(expanded);
        let db_path = root.join("palace.db");
        let identity_path = root.join("identity.txt");
        let rules_path = root.join("classifier_rules.json");
        let config_path = root.join("config.json");
        Ok(Self {
            root,
            db_path,
            identity_path,
            rules_path,
            config_path,
        })
    }

    pub fn init(&self, identity: Option<&str>) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        if !self.identity_path.exists() {
            let id = identity.unwrap_or(
                "You are my long-term coding partner. Preserve reasoning and decisions.",
            );
            fs::write(&self.identity_path, id)?;
        }
        if !self.rules_path.exists() {
            let rules = serde_json::to_string_pretty(&default_rules())?;
            fs::write(&self.rules_path, rules)?;
        }
        if !self.config_path.exists() {
            let cfg = serde_json::to_string_pretty(&default_config())?;
            fs::write(&self.config_path, cfg)?;
        }
        let conn = self.open()?;
        db::init_schema(&conn)?;
        Ok(())
    }

    pub fn open(&self) -> Result<Connection> {
        db::open(&self.db_path)
    }
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub retrieval: RetrievalConfig,
    pub mcp: McpConfig,
    pub llm: LlmConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        default_config()
    }
}

fn default_rrf_k() -> f64 {
    60.0
}

fn default_rrf_weight() -> f64 {
    18.0
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct RetrievalConfig {
    pub lexical_weight: f64,
    pub vector_weight: f64,
    #[serde(default = "default_rrf_k")]
    pub rrf_k: f64,
    #[serde(default = "default_rrf_weight")]
    pub rrf_weight: f64,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct McpConfig {
    pub quiet_default: bool,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, Default)]
pub struct LlmConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub base_url: Option<String>,
    /// Inline API key (discouraged). Never logged.
    #[serde(default)]
    pub api_key: Option<String>,
    /// Read API key from this environment variable name (preferred).
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

pub fn default_config() -> AppConfig {
    AppConfig {
        retrieval: RetrievalConfig {
            lexical_weight: 1.0,
            vector_weight: 1.3,
            rrf_k: 60.0,
            rrf_weight: 18.0,
        },
        mcp: McpConfig {
            quiet_default: true,
        },
        llm: LlmConfig::default(),
    }
}

pub fn load_config(path: &Path) -> AppConfig {
    let Ok(raw) = fs::read_to_string(path) else {
        return default_config();
    };
    serde_json::from_str(&raw).unwrap_or_else(|_| default_config())
}

pub fn mine_path(
    conn: &Connection,
    target: &Path,
    rules_path: &Path,
    override_wing: Option<&str>,
    override_hall: Option<&str>,
    override_room: Option<&str>,
    bank_id: Option<&str>,
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
        let bank = bank_id.unwrap_or("default");
        let source_path = entry.path().to_string_lossy().to_string();
        let content_hash = sha256_hex(&source_path, content_trimmed);
        let exists = conn
            .query_row(
                "SELECT 1 FROM drawers WHERE bank_id = ?1 AND content_hash = ?2 LIMIT 1",
                params![bank, content_hash],
                |_| Ok(1i64),
            )
            .optional()?
            .is_some();
        if exists {
            continue;
        }
        let now = now_rfc3339()?;
        conn.execute(
            "INSERT INTO drawers(wing, hall, room, source_path, content, content_hash, bank_id, created_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![wing, hall, room, source_path, content_trimmed, content_hash, bank, now],
        )?;
        let row_id = conn.last_insert_rowid();
        upsert_vector(conn, row_id, content_trimmed)?;
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
    bank_id: Option<&str>,
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
            let bank = bank_id.unwrap_or("default");
            let source_path = format!("{}#chunk-{}", entry.path().to_string_lossy(), idx + 1);
            let content_hash = sha256_hex(&source_path, &chunk);
            let exists = conn
                .query_row(
                    "SELECT 1 FROM drawers WHERE bank_id = ?1 AND content_hash = ?2 LIMIT 1",
                    params![bank, content_hash],
                    |_| Ok(1i64),
                )
                .optional()?
                .is_some();
            if exists {
                continue;
            }
            let now = now_rfc3339()?;
            conn.execute(
                "INSERT INTO drawers(wing, hall, room, source_path, content, content_hash, bank_id, created_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![wing, hall, room, source_path, chunk.trim(), content_hash, bank, now],
            )?;
            let row_id = conn.last_insert_rowid();
            upsert_vector(conn, row_id, chunk.trim())?;
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
    bank_id: Option<&str>,
    limit: usize,
) -> Result<Vec<SearchRow>> {
    search_with_options(
        conn,
        query,
        wing,
        hall,
        room,
        bank_id,
        limit,
        &default_config().retrieval,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn search_with_options(
    conn: &Connection,
    query: &str,
    wing: Option<&str>,
    hall: Option<&str>,
    room: Option<&str>,
    bank_id: Option<&str>,
    limit: usize,
    retrieval: &RetrievalConfig,
    include_explain: bool,
) -> Result<Vec<SearchRow>> {
    let cap = limit.saturating_mul(4).max(32) as i64;
    let k_rrf = retrieval.rrf_k.max(1.0);
    let has_cjk_query = contains_cjk(query);

    let mut out = Vec::new();
    if let Some(fts_query) = build_fts_query(query) {
        let sql = r#"
            SELECT d.id, d.wing, d.hall, d.room, d.source_path, d.bank_id,
                   snippet(drawers_fts, 0, '[', ']', ' ... ', 20) AS snippet,
                   d.content
            FROM drawers_fts
            JOIN drawers d ON d.id = drawers_fts.rowid
            WHERE drawers_fts MATCH ?1
              AND (?2 IS NULL OR d.wing = ?2)
              AND (?3 IS NULL OR d.hall = ?3)
              AND (?4 IS NULL OR d.room = ?4)
              AND (?5 IS NULL OR d.bank_id = ?5)
            ORDER BY bm25(drawers_fts), d.id DESC
            LIMIT ?6
        "#;

        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query(params![fts_query, wing, hall, room, bank_id, cap])?;

        while let Some(r) = rows.next()? {
            out.push(SearchRow {
                id: r.get(0)?,
                wing: r.get(1)?,
                hall: r.get(2)?,
                room: r.get(3)?,
                source_path: r.get(4)?,
                bank_id: r.get(5)?,
                snippet: r.get(6)?,
                content: r.get(7)?,
                score: 0.0,
                rrf: 0.0,
                explain: None,
            });
        }
    }

    if out.is_empty() || has_cjk_query {
        append_like_fallback(conn, query, wing, hall, room, bank_id, cap, &mut out)?;
    }

    apply_rrf_to_candidates(query, &mut out, k_rrf);
    rerank_rows(query, &mut out, retrieval, include_explain);
    out.truncate(limit);
    Ok(out)
}

fn apply_rrf_to_candidates(query: &str, candidates: &mut [SearchRow], k_rrf: f64) {
    if candidates.is_empty() {
        return;
    }
    let n = candidates.len();
    let ranks_vec = ranks_by_cosine_order(candidates, query);
    for (rank_fts, row) in candidates.iter_mut().enumerate() {
        let r_f = (rank_fts + 1) as f64;
        let r_v = ranks_vec.get(&row.id).copied().unwrap_or(n + 1) as f64;
        row.rrf = 1.0 / (k_rrf + r_f) + 1.0 / (k_rrf + r_v);
    }
}

fn ranks_by_cosine_order(candidates: &[SearchRow], query: &str) -> HashMap<i64, usize> {
    let qvec = sparse_embedding(query);
    let mut idxs: Vec<usize> = (0..candidates.len()).collect();
    idxs.sort_by(|&i, &j| {
        let ci = cosine_sim(&qvec, &sparse_embedding(&candidates[i].content));
        let cj = cosine_sim(&qvec, &sparse_embedding(&candidates[j].content));
        cj.partial_cmp(&ci).unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut out = HashMap::new();
    for (rank0, &idx) in idxs.iter().enumerate() {
        out.insert(candidates[idx].id, rank0 + 1);
    }
    out
}

#[derive(Debug)]
pub struct SearchRow {
    pub id: i64,
    pub wing: String,
    pub hall: String,
    pub room: String,
    pub source_path: String,
    pub bank_id: String,
    pub snippet: String,
    pub content: String,
    pub score: f64,
    /// Reciprocal-rank-fusion style score from list order vs cosine order (pre-final rerank).
    pub rrf: f64,
    pub explain: Option<String>,
}

pub fn status(conn: &Connection) -> Result<Status> {
    status_for_bank(conn, None)
}

pub fn status_for_bank(conn: &Connection, bank_id: Option<&str>) -> Result<Status> {
    let drawers: i64 = conn.query_row(
        "SELECT COUNT(*) FROM drawers WHERE (?1 IS NULL OR bank_id = ?1)",
        params![bank_id],
        |r| r.get(0),
    )?;
    let tunnels: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tunnels WHERE (?1 IS NULL OR bank_id = ?1)",
        params![bank_id],
        |r| r.get(0),
    )?;
    let wings: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT wing) FROM drawers WHERE (?1 IS NULL OR bank_id = ?1)",
        params![bank_id],
        |r| r.get(0),
    )?;
    let kg_facts: i64 = conn.query_row(
        "SELECT COUNT(*) FROM kg_facts WHERE (?1 IS NULL OR bank_id = ?1)",
        params![bank_id],
        |r| r.get(0),
    )?;
    Ok(Status {
        drawers,
        wings,
        tunnels,
        kg_facts,
    })
}

pub struct Status {
    pub drawers: i64,
    pub wings: i64,
    pub tunnels: i64,
    pub kg_facts: i64,
}

pub fn banner_ascii() -> &'static str {
    r#"
    ██████╗ ██╗   ██╗███████╗████████╗      ███╗   ███╗███████╗███╗   ███╗
    ██╔══██╗██║   ██║██╔════╝╚══██╔══╝      ████╗ ████║██╔════╝████╗ ████║
    ██████╔╝██║   ██║███████╗   ██║         ██╔████╔██║█████╗  ██╔████╔██║
    ██╔══██╗██║   ██║╚════██║   ██║         ██║╚██╔╝██║██╔══╝  ██║╚██╔╝██║
    ██║  ██║╚██████╔╝███████║   ██║         ██║ ╚═╝ ██║███████╗██║ ╚═╝ ██║
    ╚═╝  ╚═╝ ╚═════╝ ╚══════╝   ╚═╝         ╚═╝     ╚═╝╚══════╝╚═╝     ╚═╝

                   Palace Memory CLI  •  local-first  •  verbatim
"#
}

pub fn banner() -> String {
    if std::env::var("NO_COLOR").is_ok() {
        return banner_ascii().to_string();
    }
    let line_colors = [93, 208, 51, 45, 39, 33, 99];
    let mut out = String::new();
    for (i, line) in banner_ascii().lines().enumerate() {
        if line.trim().is_empty() {
            out.push('\n');
            continue;
        }
        let c = line_colors[i % line_colors.len()];
        out.push_str(&format!("\x1b[38;5;{c}m{line}\x1b[0m\n"));
    }
    out
}

pub fn principles_report(conn: &Connection) -> Result<String> {
    let s = status(conn)?;
    let last_bench = latest_benchmark(conn)?;
    let mut out = String::new();
    out.push_str("Core Principles Alignment\n");
    out.push_str("- raw verbatim storage: done\n");
    out.push_str("- wing/hall/room structure: done\n");
    out.push_str("- tunnel navigation: done\n");
    out.push_str("- local-first runtime: done\n");
    out.push_str("- wake-up context layers: done (L0/L1)\n");
    out.push_str("- knowledge graph temporal facts: baseline done\n");
    out.push_str("- mcp tool interface: done (minimal set)\n");
    out.push_str("- aaak compression dialect: pending\n");
    out.push_str(&format!(
        "\nCurrent palace stats: drawers={}, wings={}, tunnels={}, kg_facts={}",
        s.drawers, s.wings, s.tunnels, s.kg_facts
    ));
    if let Some(b) = last_bench {
        let seed = b
            .seed
            .map(|seed| format!(", seed={seed}"))
            .unwrap_or_default();
        out.push_str(&format!(
            "\nLast benchmark: mode={}{}, recall@{}={:.2}%, latency_ms={}, throughput={:.2}/s",
            b.mode,
            seed,
            b.k,
            b.recall * 100.0,
            b.latency_ms,
            b.throughput_per_sec
        ));
    }
    Ok(out)
}

pub fn kg_add(
    conn: &Connection,
    subject: &str,
    predicate: &str,
    object: &str,
    valid_from: Option<&str>,
    source_drawer_id: Option<i64>,
    bank_id: Option<&str>,
) -> Result<()> {
    let now = now_rfc3339()?;
    let bank = kg_bank_for_insert(conn, bank_id, source_drawer_id)?;
    conn.execute(
        "INSERT INTO kg_facts(subject, predicate, object, valid_from, valid_to, source_drawer_id, bank_id, created_at) VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7)",
        params![subject, predicate, object, valid_from.unwrap_or(&now), source_drawer_id, bank, now],
    )?;
    Ok(())
}

fn kg_bank_for_insert(
    conn: &Connection,
    bank_id: Option<&str>,
    source_drawer_id: Option<i64>,
) -> Result<String> {
    if let Some(bank) = bank_id {
        return Ok(bank.to_string());
    }
    if let Some(drawer_id) = source_drawer_id {
        let bank = conn
            .query_row(
                "SELECT bank_id FROM drawers WHERE id = ?1",
                params![drawer_id],
                |r| r.get::<_, String>(0),
            )
            .optional()?;
        if let Some(bank) = bank {
            return Ok(bank);
        }
    }
    Ok("default".to_string())
}

pub fn drawer_content(conn: &Connection, id: i64) -> Result<Option<String>> {
    drawer_content_for_bank(conn, id, None)
}

pub fn drawer_content_for_bank(
    conn: &Connection,
    id: i64,
    bank_id: Option<&str>,
) -> Result<Option<String>> {
    let v = conn
        .query_row(
            "SELECT content FROM drawers WHERE id = ?1 AND (?2 IS NULL OR bank_id = ?2)",
            params![id, bank_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(v)
}

/// Grounded synthesis over top search hits (optional LLM).
pub fn reflect_answer(
    conn: &Connection,
    cfg: &LlmConfig,
    retrieval: &RetrievalConfig,
    query: &str,
    bank_id: Option<&str>,
    search_limit: usize,
) -> Result<String> {
    if !crate::llm::llm_ready(cfg) {
        anyhow::bail!(
            "LLM not configured: set llm.enabled=true, llm.base_url, llm.model, and api key via llm.api_key_env or llm.api_key"
        );
    }
    let rows = search_with_options(
        conn,
        query,
        None,
        None,
        None,
        bank_id,
        search_limit,
        retrieval,
        false,
    )?;
    let mut ctx = String::new();
    for r in &rows {
        let excerpt: String = r.content.chars().take(4000).collect();
        ctx.push_str(&format!(
            "--- drawer {} | {} / {} / {} | {}\n{excerpt}\n",
            r.id, r.wing, r.hall, r.room, r.source_path
        ));
    }
    let system = "You are a careful assistant. Answer using ONLY the CONTEXT below. If the context is insufficient, say you do not know.";
    let user = format!("CONTEXT:\n{ctx}\n\nQUESTION:\n{query}");
    crate::llm::chat_completion(cfg, vec![("system", system.to_string()), ("user", user)])
}

/// LLM-assisted triple extraction into `kg_facts` (optional LLM).
pub fn extract_to_kg(
    conn: &Connection,
    cfg: &LlmConfig,
    text: &str,
    bank_id: Option<&str>,
) -> Result<usize> {
    if !crate::llm::llm_ready(cfg) {
        anyhow::bail!(
            "LLM not configured: set llm.enabled=true, llm.base_url, llm.model, and api key via llm.api_key_env or llm.api_key"
        );
    }
    let system = "Extract grounded triples. Output ONLY a JSON array of objects with keys subject, predicate, and object (all strings). No markdown fences.";
    let user = format!("TEXT:\n{text}");
    let body =
        crate::llm::chat_completion(cfg, vec![("system", system.to_string()), ("user", user)])?;
    let triples = crate::llm::parse_kg_triples_json(&body)?;
    let mut n = 0usize;
    for t in triples {
        let s = t.subject.trim();
        let p = t.predicate.trim();
        let o = t.object.trim();
        if s.is_empty() || p.is_empty() || o.is_empty() {
            continue;
        }
        kg_add(conn, s, p, o, None, None, bank_id)?;
        n += 1;
    }
    Ok(n)
}

pub fn kg_query(
    conn: &Connection,
    subject: &str,
    as_of: Option<&str>,
    bank_id: Option<&str>,
) -> Result<Vec<KgFact>> {
    let as_of_ts = match as_of {
        Some(value) => value.to_string(),
        None => now_rfc3339()?,
    };
    let mut stmt = conn.prepare(
        r#"
        SELECT id, subject, predicate, object, valid_from, valid_to, source_drawer_id, bank_id
        FROM kg_facts
        WHERE subject = ?1
          AND valid_from <= ?2
          AND (valid_to IS NULL OR valid_to > ?2)
          AND (?3 IS NULL OR bank_id = ?3)
        ORDER BY valid_from DESC, id DESC
    "#,
    )?;
    let mut rows = stmt.query(params![subject, as_of_ts, bank_id])?;
    let mut out = Vec::new();
    while let Some(r) = rows.next()? {
        out.push(KgFact {
            id: r.get(0)?,
            subject: r.get(1)?,
            predicate: r.get(2)?,
            object: r.get(3)?,
            valid_from: r.get(4)?,
            valid_to: r.get(5)?,
            source_drawer_id: r.get(6)?,
            bank_id: r.get(7)?,
        });
    }
    Ok(out)
}

pub fn kg_invalidate(
    conn: &Connection,
    subject: &str,
    predicate: &str,
    object: &str,
    ended: Option<&str>,
    bank_id: Option<&str>,
) -> Result<usize> {
    let ended_at = match ended {
        Some(value) => value.to_string(),
        None => now_rfc3339()?,
    };
    let changed = conn.execute(
        "UPDATE kg_facts SET valid_to = ?1 WHERE subject = ?2 AND predicate = ?3 AND object = ?4 AND valid_to IS NULL AND (?5 IS NULL OR bank_id = ?5)",
        params![ended_at, subject, predicate, object, bank_id],
    )?;
    Ok(changed)
}

pub struct KgFact {
    pub id: i64,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub valid_from: String,
    pub valid_to: Option<String>,
    pub source_drawer_id: Option<i64>,
    pub bank_id: String,
}

pub fn kg_timeline(conn: &Connection, subject: &str, bank_id: Option<&str>) -> Result<Vec<KgFact>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT id, subject, predicate, object, valid_from, valid_to, source_drawer_id, bank_id
        FROM kg_facts
        WHERE subject = ?1
          AND (?2 IS NULL OR bank_id = ?2)
        ORDER BY valid_from ASC, id ASC
    "#,
    )?;
    let mut rows = stmt.query(params![subject, bank_id])?;
    let mut out = Vec::new();
    while let Some(r) = rows.next()? {
        out.push(KgFact {
            id: r.get(0)?,
            subject: r.get(1)?,
            predicate: r.get(2)?,
            object: r.get(3)?,
            valid_from: r.get(4)?,
            valid_to: r.get(5)?,
            source_drawer_id: r.get(6)?,
            bank_id: r.get(7)?,
        });
    }
    Ok(out)
}

pub struct KgStats {
    pub facts: i64,
    pub subjects: i64,
    pub predicates: i64,
    pub active_facts: i64,
}

pub fn kg_stats(conn: &Connection, bank_id: Option<&str>) -> Result<KgStats> {
    let facts: i64 = conn.query_row(
        "SELECT COUNT(*) FROM kg_facts WHERE (?1 IS NULL OR bank_id = ?1)",
        params![bank_id],
        |r| r.get(0),
    )?;
    let subjects: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT subject) FROM kg_facts WHERE (?1 IS NULL OR bank_id = ?1)",
        params![bank_id],
        |r| r.get(0),
    )?;
    let predicates: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT predicate) FROM kg_facts WHERE (?1 IS NULL OR bank_id = ?1)",
        params![bank_id],
        |r| r.get(0),
    )?;
    let active_facts: i64 = conn.query_row(
        "SELECT COUNT(*) FROM kg_facts WHERE valid_to IS NULL AND (?1 IS NULL OR bank_id = ?1)",
        params![bank_id],
        |r| r.get(0),
    )?;
    Ok(KgStats {
        facts,
        subjects,
        predicates,
        active_facts,
    })
}

pub struct KgConflict {
    pub subject: String,
    pub predicate: String,
    pub objects: Vec<String>,
}

pub fn kg_conflicts(conn: &Connection) -> Result<Vec<KgConflict>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT subject, predicate, GROUP_CONCAT(DISTINCT object) AS objects, COUNT(DISTINCT object) AS n
        FROM kg_facts
        WHERE valid_to IS NULL
        GROUP BY subject, predicate
        HAVING n > 1
    "#,
    )?;
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(r) = rows.next()? {
        let obj_csv: String = r.get(2)?;
        out.push(KgConflict {
            subject: r.get(0)?,
            predicate: r.get(1)?,
            objects: obj_csv.split(',').map(|s| s.to_string()).collect(),
        });
    }
    Ok(out)
}

pub fn taxonomy(conn: &Connection, bank_id: Option<&str>) -> Result<Vec<TaxonomyRow>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT wing, hall, room, COUNT(*) AS cnt
        FROM drawers
        WHERE (?1 IS NULL OR bank_id = ?1)
        GROUP BY wing, hall, room
        ORDER BY wing, hall, cnt DESC, room
    "#,
    )?;
    let mut rows = stmt.query(params![bank_id])?;
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

pub fn traverse(
    conn: &Connection,
    wing: &str,
    room: &str,
    bank_id: Option<&str>,
) -> Result<Vec<TraverseEdge>> {
    let mut out = Vec::new();
    let mut explicit = conn.prepare(
        r#"
        SELECT from_wing, from_room, to_wing, to_room, 'explicit'
        FROM tunnels
        WHERE ((from_wing = ?1 AND from_room = ?2) OR (to_wing = ?1 AND to_room = ?2))
          AND (?3 IS NULL OR bank_id = ?3)
    "#,
    )?;
    let mut rows = explicit.query(params![wing, room, bank_id])?;
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
          AND (?3 IS NULL OR bank_id = ?3)
    "#,
    )?;
    let mut rows = implicit.query(params![wing, room, bank_id])?;
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

#[derive(Debug)]
pub struct TraverseEdge {
    pub from_wing: String,
    pub from_room: String,
    pub to_wing: String,
    pub to_room: String,
    pub kind: String,
}

pub fn benchmark_run(
    conn: &Connection,
    samples: usize,
    k: usize,
    mode: &str,
    seed: Option<u64>,
) -> Result<BenchmarkResult> {
    let mut stmt = conn.prepare("SELECT id, content FROM drawers ORDER BY id DESC LIMIT 10000")?;
    let mut rows = stmt.query([])?;
    let mut corpus = Vec::new();
    while let Some(r) = rows.next()? {
        corpus.push((r.get::<_, i64>(0)?, r.get::<_, String>(1)?));
    }
    if corpus.is_empty() {
        return Ok(BenchmarkResult {
            total: 0,
            hits: 0,
            recall: 0.0,
            k,
            mode: mode.to_string(),
            seed: if mode == "random" { seed } else { None },
            latency_ms: 0,
            throughput_per_sec: 0.0,
        });
    }
    let chosen = choose_benchmark_corpus(corpus, samples, mode, seed);
    let start = Instant::now();
    let mut total = 0usize;
    let mut hits = 0usize;
    for (id, content) in chosen {
        let query = content
            .split_whitespace()
            .take(8)
            .collect::<Vec<_>>()
            .join(" ");
        if query.trim().is_empty() {
            continue;
        }
        total += 1;
        let result = search(conn, &query, None, None, None, None, k)?;
        if result.iter().any(|row| row.id == id) {
            hits += 1;
        }
    }
    let elapsed = start.elapsed();
    let latency_ms = elapsed.as_millis() as u64;
    let throughput_per_sec = if elapsed.as_secs_f64() == 0.0 {
        total as f64
    } else {
        total as f64 / elapsed.as_secs_f64()
    };
    let recall = if total == 0 {
        0.0
    } else {
        hits as f64 / total as f64
    };
    let out = BenchmarkResult {
        total,
        hits,
        recall,
        k,
        mode: mode.to_string(),
        seed: if mode == "random" { seed } else { None },
        latency_ms,
        throughput_per_sec,
    };
    let seed_db = out
        .seed
        .map(i64::try_from)
        .transpose()
        .context("benchmark seed exceeds SQLite INTEGER range")?;
    conn.execute(
        "INSERT INTO benchmark_runs(mode, samples, top_k, recall, latency_ms, throughput_per_sec, hits, seed, created_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            out.mode,
            out.total as i64,
            out.k as i64,
            out.recall,
            out.latency_ms as i64,
            out.throughput_per_sec,
            out.hits as i64,
            seed_db,
            now_rfc3339()?
        ],
    )?;
    Ok(out)
}

fn choose_benchmark_corpus(
    mut corpus: Vec<(i64, String)>,
    samples: usize,
    mode: &str,
    seed: Option<u64>,
) -> Vec<(i64, String)> {
    if mode == "random" {
        if let Some(seed) = seed {
            let mut rng = StdRng::seed_from_u64(seed);
            corpus.shuffle(&mut rng);
        } else {
            let mut rng = rand::rng();
            corpus.shuffle(&mut rng);
        }
    }
    corpus.into_iter().take(samples).collect()
}

pub struct BenchmarkResult {
    pub total: usize,
    pub hits: usize,
    pub recall: f64,
    pub k: usize,
    pub mode: String,
    pub seed: Option<u64>,
    pub latency_ms: u64,
    pub throughput_per_sec: f64,
}

pub fn latest_benchmark(conn: &Connection) -> Result<Option<BenchmarkResult>> {
    let row = conn
        .query_row(
            "SELECT mode, samples, top_k, recall, latency_ms, throughput_per_sec, hits, seed FROM benchmark_runs ORDER BY id DESC LIMIT 1",
            [],
            |r| {
                let seed = r
                    .get::<_, Option<i64>>(7)?
                    .and_then(|value| u64::try_from(value).ok());
                Ok(BenchmarkResult {
                    mode: r.get(0)?,
                    total: r.get::<_, i64>(1)? as usize,
                    k: r.get::<_, i64>(2)? as usize,
                    recall: r.get(3)?,
                    latency_ms: r.get::<_, i64>(4)? as u64,
                    throughput_per_sec: r.get(5)?,
                    hits: r.get::<_, i64>(6)? as usize,
                    seed,
                })
            },
        )
        .optional()?;
    Ok(row)
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
            mode: result.mode.clone(),
            seed: result.seed,
            total: result.total,
            hits: result.hits,
            recall: result.recall,
            top_k: result.k,
            latency_ms: result.latency_ms,
            throughput_per_sec: result.throughput_per_sec,
            generated_at: now_rfc3339()?,
        };
        fs::write(report_path, serde_json::to_string_pretty(&payload)?)?;
    } else {
        let body = format!(
            "# Benchmark Report\n\n- generated_at: {}\n- mode: {}\n- seed: {}\n- samples: {}\n- hits: {}\n- recall@{}: {:.2}%\n- latency_ms: {}\n- throughput_per_sec: {:.2}\n",
            now_rfc3339()?,
            result.mode,
            result
                .seed
                .map(|seed| seed.to_string())
                .unwrap_or_else(|| "none".to_string()),
            result.total,
            result.hits,
            result.k,
            result.recall * 100.0,
            result.latency_ms,
            result.throughput_per_sec
        );
        fs::write(report_path, body)?;
    }
    Ok(())
}

fn now_rfc3339() -> Result<String> {
    Ok(OffsetDateTime::now_utc().format(&Rfc3339)?)
}

pub fn wake_up(
    conn: &Connection,
    identity_path: &Path,
    wing: Option<&str>,
    bank_id: Option<&str>,
) -> Result<String> {
    let identity = fs::read_to_string(identity_path).unwrap_or_default();
    let mut out = String::new();
    out.push_str("# L0 Identity\n");
    out.push_str(identity.trim());
    out.push_str("\n\n# L1 Critical Facts\n");
    for hall in KNOWN_HALLS {
        let mut stmt = conn.prepare(
            r#"SELECT room, substr(content, 1, 180) FROM drawers
               WHERE hall = ?1
                 AND (?2 IS NULL OR bank_id = ?2)
                 AND (?3 IS NULL OR wing = ?3)
               ORDER BY id DESC LIMIT 2"#,
        )?;
        let mut rows = stmt.query(params![hall, bank_id, wing])?;
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

fn rerank_rows(
    query: &str,
    rows: &mut [SearchRow],
    retrieval: &RetrievalConfig,
    include_explain: bool,
) {
    let terms = relevance_terms(query);
    let qvec = sparse_embedding(query);
    for row in rows.iter_mut() {
        let lexical = simple_relevance_score(&row.content, &terms, row.id);
        let dvec = sparse_embedding(&row.content);
        let vector = cosine_sim(&qvec, &dvec);
        let final_score = retrieval.rrf_weight * row.rrf
            + (lexical * retrieval.lexical_weight)
            + (vector * retrieval.vector_weight);
        row.score = final_score;
        if include_explain {
            row.explain = Some(format!(
                "rrf={:.5}, rw={:.2}, lexical={:.4}, vector={:.4}, lw={:.2}, vw={:.2}",
                row.rrf,
                retrieval.rrf_weight,
                lexical,
                vector,
                retrieval.lexical_weight,
                retrieval.vector_weight
            ));
        }
    }
    rows.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn simple_relevance_score(content: &str, terms: &[String], id: i64) -> f64 {
    let lc = unicode_lower(content);
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

fn build_fts_query(query: &str) -> Option<String> {
    let tokens: Vec<String> = unicode_tokens(query)
        .into_iter()
        .map(|s| format!("\"{}\"", s.replace('"', "\"\"")))
        .collect();
    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" "))
    }
}

#[allow(clippy::too_many_arguments)]
fn append_like_fallback(
    conn: &Connection,
    query: &str,
    wing: Option<&str>,
    hall: Option<&str>,
    room: Option<&str>,
    bank_id: Option<&str>,
    cap: i64,
    out: &mut Vec<SearchRow>,
) -> Result<()> {
    let patterns = like_fallback_patterns(query);
    if patterns.is_empty() {
        return Ok(());
    }

    let like_clause = std::iter::repeat_n("content LIKE ? ESCAPE '\\'", patterns.len())
        .collect::<Vec<_>>()
        .join(" OR ");
    let score_clause = patterns
        .iter()
        .enumerate()
        .map(|(idx, pattern)| {
            format!(
                "CASE WHEN content LIKE ? ESCAPE '\\' THEN {} ELSE 0 END",
                like_pattern_score(pattern, idx)
            )
        })
        .collect::<Vec<_>>()
        .join(" + ");
    let mut sql = format!(
        "SELECT id, wing, hall, room, source_path, bank_id, substr(content, 1, 220), content \
         FROM drawers WHERE ({like_clause})"
    );
    let like_values: Vec<rusqlite::types::Value> = patterns
        .iter()
        .map(|pattern| rusqlite::types::Value::Text(format!("%{}%", escape_like(pattern))))
        .collect();
    let mut values = like_values.clone();

    append_optional_filter(&mut sql, &mut values, "wing", wing);
    append_optional_filter(&mut sql, &mut values, "hall", hall);
    append_optional_filter(&mut sql, &mut values, "room", room);
    append_optional_filter(&mut sql, &mut values, "bank_id", bank_id);
    sql.push_str(" ORDER BY ");
    sql.push_str(&score_clause);
    sql.push_str(" DESC, id DESC LIMIT ?");
    values.extend(like_values);
    values.push(rusqlite::types::Value::Integer(cap));

    let existing: std::collections::HashSet<i64> = out.iter().map(|row| row.id).collect();
    let mut fallback = conn.prepare(&sql)?;
    let mut rows = fallback.query(rusqlite::params_from_iter(values.iter()))?;
    while let Some(r) = rows.next()? {
        let id: i64 = r.get(0)?;
        if existing.contains(&id) || out.iter().any(|row| row.id == id) {
            continue;
        }
        let snip: String = r.get(6)?;
        out.push(SearchRow {
            id,
            wing: r.get(1)?,
            hall: r.get(2)?,
            room: r.get(3)?,
            source_path: r.get(4)?,
            bank_id: r.get(5)?,
            snippet: snip,
            content: r.get(7)?,
            score: 0.0,
            rrf: 0.0,
            explain: None,
        });
    }
    Ok(())
}

fn append_optional_filter(
    sql: &mut String,
    values: &mut Vec<rusqlite::types::Value>,
    column: &str,
    value: Option<&str>,
) {
    if let Some(value) = value {
        sql.push_str(" AND ");
        sql.push_str(column);
        sql.push_str(" = ?");
        values.push(rusqlite::types::Value::Text(value.to_string()));
    }
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn like_pattern_score(pattern: &str, idx: usize) -> usize {
    let length_weight = pattern.chars().count().saturating_mul(10);
    length_weight.saturating_add(100usize.saturating_sub(idx.min(100)))
}

fn like_fallback_patterns(query: &str) -> Vec<String> {
    let mut out = Vec::new();
    push_unique(&mut out, unicode_lower(query.trim()));
    for token in unicode_tokens(query) {
        push_unique(&mut out, token.clone());
        if contains_cjk(&token) {
            for gram in char_ngrams(&token, 2)
                .into_iter()
                .chain(char_ngrams(&token, 3))
            {
                push_unique(&mut out, gram);
            }
        }
    }
    out.into_iter()
        .filter(|s| s.chars().count() >= 2)
        .take(24)
        .collect()
}

fn relevance_terms(query: &str) -> Vec<String> {
    let mut out = unicode_tokens(query);
    for token in out.clone() {
        if contains_cjk(&token) {
            for gram in char_ngrams(&token, 2)
                .into_iter()
                .chain(char_ngrams(&token, 3))
            {
                push_unique(&mut out, gram);
            }
        }
    }
    out
}

fn unicode_tokens(query: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in query.chars() {
        if ch.is_alphanumeric() {
            cur.extend(ch.to_lowercase());
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn unicode_lower(s: &str) -> String {
    s.chars().flat_map(|ch| ch.to_lowercase()).collect()
}

fn contains_cjk(s: &str) -> bool {
    s.chars().any(|ch| {
        matches!(
            ch as u32,
            0x3400..=0x4DBF
                | 0x4E00..=0x9FFF
                | 0xF900..=0xFAFF
                | 0x20000..=0x2A6DF
                | 0x2A700..=0x2B73F
                | 0x2B740..=0x2B81F
                | 0x2B820..=0x2CEAF
                | 0x3040..=0x30FF
                | 0xAC00..=0xD7AF
        )
    })
}

fn char_ngrams(s: &str, n: usize) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    if n == 0 || chars.len() < n {
        return Vec::new();
    }
    (0..=(chars.len() - n))
        .map(|i| chars[i..i + n].iter().collect())
        .collect()
}

fn push_unique(out: &mut Vec<String>, value: String) {
    if !value.is_empty() && !out.iter().any(|existing| existing == &value) {
        out.push(value);
    }
}

pub fn upsert_vector(conn: &Connection, drawer_id: i64, content: &str) -> Result<()> {
    let emb = sparse_embedding(content);
    let json = serde_json::to_string(&emb)?;
    conn.execute(
        "INSERT INTO drawer_vectors(drawer_id, vector_json) VALUES(?1, ?2)
         ON CONFLICT(drawer_id) DO UPDATE SET vector_json = excluded.vector_json",
        params![drawer_id, json],
    )?;
    Ok(())
}

pub fn sparse_embedding(text: &str) -> BTreeMap<String, f64> {
    let mut map = BTreeMap::new();
    for token in unicode_tokens(text) {
        if token.chars().count() > 2 {
            *map.entry(token.clone()).or_insert(0.0) += 1.0;
        }
        if contains_cjk(&token) {
            for gram in char_ngrams(&token, 2) {
                *map.entry(format!("cjk:{gram}")).or_insert(0.0) += 1.0;
            }
        }
    }
    let norm = map.values().map(|v| v * v).sum::<f64>().sqrt();
    if norm > 0.0 {
        for v in map.values_mut() {
            *v /= norm;
        }
    }
    map
}

fn cosine_sim(a: &BTreeMap<String, f64>, b: &BTreeMap<String, f64>) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let mut s = 0.0;
    let (small, large) = if a.len() < b.len() { (a, b) } else { (b, a) };
    for (k, v) in small {
        if let Some(v2) = large.get(k) {
            s += v * v2;
        }
    }
    s
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
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

fn trigrams(s: &str) -> std::collections::HashSet<String> {
    let clean = s
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .flat_map(|ch| ch.to_lowercase())
        .collect::<String>();
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
    mode: String,
    seed: Option<u64>,
    total: usize,
    hits: usize,
    recall: f64,
    top_k: usize,
    latency_ms: u64,
    throughput_per_sec: f64,
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

pub fn split_mega_file(
    path: &Path,
    marker: &str,
    min_lines: usize,
    dry_run: bool,
) -> Result<usize> {
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
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("session");
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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn fts_query_quotes_user_tokens() {
        assert_eq!(
            build_fts_query("hello OR world").as_deref(),
            Some("\"hello\" \"or\" \"world\"")
        );
        assert_eq!(
            build_fts_query("数据库一致性").as_deref(),
            Some("\"数据库一致性\"")
        );
        assert_eq!(build_fts_query("!!!"), None);
    }

    #[test]
    fn cjk_fallback_patterns_include_ngrams() {
        let patterns = like_fallback_patterns("数据一致性");
        assert!(patterns.contains(&"数据一致性".to_string()));
        assert!(patterns.contains(&"一致性".to_string()));
        assert!(patterns.contains(&"数据".to_string()));
    }

    #[test]
    fn sparse_embedding_preserves_cjk_ngrams() {
        let emb = sparse_embedding("数据库一致性");
        assert!(emb.contains_key("数据库一致性"));
        assert!(emb.contains_key("cjk:一致"));
    }

    #[test]
    fn cjk_fallback_prioritizes_exact_hits_before_cap() {
        let conn = Connection::open_in_memory().expect("open memory db");
        crate::db::init_schema(&conn).expect("init schema");
        let now = "2024-01-01T00:00:00Z";

        conn.execute(
            "INSERT INTO drawers(wing, hall, room, source_path, content, content_hash, bank_id, created_at)
             VALUES('w', 'h', 'r', 'exact.md', '这里有一致性数据可靠证据', 'exact-hash', 'default', ?1)",
            params![now],
        )
        .expect("insert exact cjk drawer");
        for idx in 0..48 {
            conn.execute(
                "INSERT INTO drawers(wing, hall, room, source_path, content, content_hash, bank_id, created_at)
                 VALUES('w', 'h', 'r', ?1, ?2, ?3, 'default', ?4)",
                params![
                    format!("broad-{idx}.md"),
                    format!("数据 broad match {idx}"),
                    format!("broad-hash-{idx}"),
                    now
                ],
            )
            .expect("insert broad cjk drawer");
        }

        let rows =
            search(&conn, "一致性数据", None, None, None, None, 1).expect("search cjk fallback");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].source_path, "exact.md");
    }

    #[test]
    fn seeded_random_benchmark_selection_is_repeatable() {
        let corpus = (1..=10)
            .map(|id| (id, format!("drawer {id}")))
            .collect::<Vec<_>>();

        let first = choose_benchmark_corpus(corpus.clone(), 5, "random", Some(42));
        let second = choose_benchmark_corpus(corpus.clone(), 5, "random", Some(42));
        let fixed = choose_benchmark_corpus(corpus.clone(), 5, "fixed", Some(42));

        assert_eq!(first, second);
        assert_eq!(
            fixed.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn kg_queries_and_stats_are_bank_scoped() {
        let conn = Connection::open_in_memory().expect("open memory db");
        crate::db::init_schema(&conn).expect("init schema");
        let now = "2024-01-01T00:00:00Z";

        conn.execute(
            "INSERT INTO drawers(wing, hall, room, source_path, content, content_hash, bank_id, created_at)
             VALUES('w', 'h', 'r1', 'a.md', 'alpha', 'hash-a', 'bank_a', ?1)",
            params![now],
        )
        .expect("insert bank_a drawer");
        let bank_a_drawer = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO drawers(wing, hall, room, source_path, content, content_hash, bank_id, created_at)
             VALUES('w', 'h', 'r2', 'b.md', 'beta', 'hash-b', 'bank_b', ?1)",
            params![now],
        )
        .expect("insert bank_b drawer");
        let bank_b_drawer = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO drawers(wing, hall, room, source_path, content, content_hash, bank_id, created_at)
             VALUES('w', 'h', 'r2', 'a2.md', 'alpha 2', 'hash-a2', 'bank_a', ?1)",
            params![now],
        )
        .expect("insert bank_a coordinate-collision drawer");
        conn.execute(
            "INSERT INTO drawers(wing, hall, room, source_path, content, content_hash, bank_id, created_at)
             VALUES('w', 'h', 'r1', 'b2.md', 'beta 2', 'hash-b2', 'bank_b', ?1)",
            params![now],
        )
        .expect("insert bank_b coordinate-collision drawer");

        kg_add(
            &conn,
            "Subject",
            "likes",
            "A",
            Some(now),
            Some(bank_a_drawer),
            None,
        )
        .expect("add bank_a fact");
        kg_add(
            &conn,
            "Subject",
            "likes",
            "B",
            Some(now),
            Some(bank_b_drawer),
            None,
        )
        .expect("add bank_b fact");

        let a = kg_query(&conn, "Subject", None, Some("bank_a")).expect("query bank_a");
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].object, "A");
        assert_eq!(a[0].bank_id, "bank_a");

        let b = kg_timeline(&conn, "Subject", Some("bank_b")).expect("timeline bank_b");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].object, "B");
        assert_eq!(b[0].bank_id, "bank_b");

        let all = kg_query(&conn, "Subject", None, None).expect("query all banks");
        assert_eq!(all.len(), 2);

        let stats_a = kg_stats(&conn, Some("bank_a")).expect("stats bank_a");
        assert_eq!(stats_a.facts, 1);
        assert_eq!(stats_a.subjects, 1);
        assert_eq!(stats_a.predicates, 1);

        let status_b = status_for_bank(&conn, Some("bank_b")).expect("status bank_b");
        assert_eq!(status_b.drawers, 2);
        assert_eq!(status_b.kg_facts, 1);

        conn.execute(
            "INSERT INTO tunnels(from_wing, from_room, to_wing, to_room, bank_id, created_at)
             VALUES('w', 'r1', 'w', 'r2', 'bank_b', ?1)",
            params![now],
        )
        .expect("insert bank_b tunnel");
        let scoped_traverse = traverse(&conn, "w", "r1", Some("bank_a")).expect("traverse bank_a");
        assert!(
            scoped_traverse.iter().all(|edge| edge.kind != "explicit"),
            "bank_b explicit tunnel must not leak into bank_a even when room coordinates collide: {scoped_traverse:?}"
        );
        let bank_b_traverse = traverse(&conn, "w", "r1", Some("bank_b")).expect("traverse bank_b");
        assert!(
            bank_b_traverse.iter().any(|edge| edge.kind == "explicit"),
            "bank_b traverse should see its own explicit tunnel"
        );
        let global_traverse = traverse(&conn, "w", "r1", None).expect("traverse global");
        assert!(
            global_traverse.iter().any(|edge| edge.kind == "explicit"),
            "global traverse should keep legacy explicit tunnel visibility"
        );
        assert_eq!(
            status_for_bank(&conn, Some("bank_a"))
                .expect("status bank_a")
                .tunnels,
            0
        );
        assert_eq!(
            status_for_bank(&conn, Some("bank_b"))
                .expect("status bank_b")
                .tunnels,
            1
        );

        assert!(
            drawer_content_for_bank(&conn, bank_b_drawer, Some("bank_a"))
                .expect("bank-filtered drawer read")
                .is_none()
        );
        assert_eq!(
            drawer_content_for_bank(&conn, bank_b_drawer, Some("bank_b"))
                .expect("bank-filtered drawer read"),
            Some("beta".to_string())
        );

        let changed = kg_invalidate(&conn, "Subject", "likes", "A", Some(now), Some("bank_b"))
            .expect("invalidate other bank");
        assert_eq!(changed, 0);
        let changed = kg_invalidate(&conn, "Subject", "likes", "A", Some(now), Some("bank_a"))
            .expect("invalidate bank_a");
        assert_eq!(changed, 1);
    }

    #[test]
    fn mine_path_dedupes_by_bank_and_content_hash() {
        let conn = Connection::open_in_memory().expect("open memory db");
        crate::db::init_schema(&conn).expect("init schema");
        let root = unique_test_dir("mine-bank-dedupe");
        std::fs::create_dir_all(&root).expect("create temp dir");
        let note = root.join("same.md");
        std::fs::write(&note, "same content for both banks").expect("write note");
        let rules = root.join("missing-rules.json");

        assert_eq!(
            mine_path(&conn, &root, &rules, None, None, None, Some("bank_a")).expect("mine bank_a"),
            1
        );
        assert_eq!(
            mine_path(&conn, &root, &rules, None, None, None, Some("bank_b")).expect("mine bank_b"),
            1
        );
        assert_eq!(
            mine_path(&conn, &root, &rules, None, None, None, Some("bank_a"))
                .expect("remine bank_a"),
            0
        );

        let bank_a: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM drawers WHERE bank_id = 'bank_a'",
                [],
                |r| r.get(0),
            )
            .expect("bank_a count");
        let bank_b: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM drawers WHERE bank_id = 'bank_b'",
                [],
                |r| r.get(0),
            )
            .expect("bank_b count");
        assert_eq!(bank_a, 1);
        assert_eq!(bank_b, 1);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn mine_path_convos_dedupes_by_bank_and_content_hash() {
        let conn = Connection::open_in_memory().expect("open memory db");
        crate::db::init_schema(&conn).expect("init schema");
        let root = unique_test_dir("mine-convos-bank-dedupe");
        std::fs::create_dir_all(&root).expect("create temp dir");
        let convo = root.join("chat.txt");
        std::fs::write(
            &convo,
            "User: remember this same conversation\nAssistant: same answer",
        )
        .expect("write convo");
        let rules = root.join("missing-rules.json");

        assert_eq!(
            mine_path_convos(&conn, &root, &rules, None, None, None, Some("bank_a"))
                .expect("mine convos bank_a"),
            1
        );
        assert_eq!(
            mine_path_convos(&conn, &root, &rules, None, None, None, Some("bank_b"))
                .expect("mine convos bank_b"),
            1
        );
        assert_eq!(
            mine_path_convos(&conn, &root, &rules, None, None, None, Some("bank_a"))
                .expect("remine convos bank_a"),
            0
        );

        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM drawers", [], |r| r.get(0))
            .expect("total count");
        assert_eq!(total, 2);

        let _ = std::fs::remove_dir_all(root);
    }

    fn unique_test_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "rust-mempalace-{name}-{}-{nanos}",
            std::process::id()
        ))
    }
}
