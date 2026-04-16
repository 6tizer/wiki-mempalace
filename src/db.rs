use anyhow::Result;
use rusqlite::{Connection, params};
use std::path::Path;

pub fn open(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(conn)
}

pub fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS drawers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            wing TEXT NOT NULL,
            hall TEXT NOT NULL,
            room TEXT NOT NULL,
            source_path TEXT NOT NULL,
            content TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            bank_id TEXT NOT NULL DEFAULT 'default',
            created_at TEXT NOT NULL
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_drawers_hash ON drawers(content_hash);
        CREATE INDEX IF NOT EXISTS idx_drawers_whr ON drawers(wing, hall, room);

        CREATE TABLE IF NOT EXISTS tunnels (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            from_wing TEXT NOT NULL,
            from_room TEXT NOT NULL,
            to_wing TEXT NOT NULL,
            to_room TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS kg_facts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            subject TEXT NOT NULL,
            predicate TEXT NOT NULL,
            object TEXT NOT NULL,
            valid_from TEXT NOT NULL,
            valid_to TEXT,
            source_drawer_id INTEGER,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_kg_spo ON kg_facts(subject, predicate, object);
        CREATE INDEX IF NOT EXISTS idx_kg_subject ON kg_facts(subject);

        CREATE TABLE IF NOT EXISTS drawer_vectors (
            drawer_id INTEGER PRIMARY KEY,
            vector_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS benchmark_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            mode TEXT NOT NULL,
            samples INTEGER NOT NULL,
            top_k INTEGER NOT NULL,
            recall REAL NOT NULL,
            latency_ms INTEGER NOT NULL,
            throughput_per_sec REAL NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS drawers_fts
        USING fts5(content, wing, hall, room, source_path, content='drawers', content_rowid='id');

        CREATE TRIGGER IF NOT EXISTS drawers_ai AFTER INSERT ON drawers BEGIN
            INSERT INTO drawers_fts(rowid, content, wing, hall, room, source_path)
            VALUES (new.id, new.content, new.wing, new.hall, new.room, new.source_path);
        END;

        CREATE TRIGGER IF NOT EXISTS drawers_ad AFTER DELETE ON drawers BEGIN
            INSERT INTO drawers_fts(drawers_fts, rowid, content) VALUES('delete', old.id, old.content);
        END;

        CREATE TRIGGER IF NOT EXISTS drawers_au AFTER UPDATE ON drawers BEGIN
            INSERT INTO drawers_fts(drawers_fts, rowid, content) VALUES('delete', old.id, old.content);
            INSERT INTO drawers_fts(rowid, content, wing, hall, room, source_path)
            VALUES (new.id, new.content, new.wing, new.hall, new.room, new.source_path);
        END;
    "#,
    )?;
    migrate_schema(conn)?;
    Ok(())
}

/// Apply additive migrations for existing palace DBs created before new columns.
pub fn migrate_schema(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(drawers)")?;
    let mut has_bank = false;
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        let name: String = r.get(1)?;
        if name == "bank_id" {
            has_bank = true;
            break;
        }
    }
    if !has_bank {
        conn.execute(
            "ALTER TABLE drawers ADD COLUMN bank_id TEXT NOT NULL DEFAULT 'default'",
            [],
        )?;
    }
    Ok(())
}

pub fn insert_tunnel(
    conn: &Connection,
    from_wing: &str,
    from_room: &str,
    to_wing: &str,
    to_room: &str,
    created_at: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO tunnels(from_wing, from_room, to_wing, to_room, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![from_wing, from_room, to_wing, to_room, created_at],
    )?;
    Ok(())
}
