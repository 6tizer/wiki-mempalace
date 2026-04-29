use anyhow::Result;
use rusqlite::{params, Connection};
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
            bank_id TEXT NOT NULL DEFAULT 'default',
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
            bank_id TEXT NOT NULL DEFAULT 'default',
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
            seed INTEGER,
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

    // Add hits column to benchmark_runs if missing (existing DBs have hits = 0 for old rows)
    let mut bstmt = conn.prepare("PRAGMA table_info(benchmark_runs)")?;
    let mut has_hits = false;
    let mut brows = bstmt.query([])?;
    while let Some(r) = brows.next()? {
        let name: String = r.get(1)?;
        if name == "hits" {
            has_hits = true;
            break;
        }
    }
    if !has_hits {
        conn.execute(
            "ALTER TABLE benchmark_runs ADD COLUMN hits INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }

    // Optional deterministic random seed for benchmark reproducibility.
    let mut bstmt = conn.prepare("PRAGMA table_info(benchmark_runs)")?;
    let mut has_seed = false;
    let mut brows = bstmt.query([])?;
    while let Some(r) = brows.next()? {
        let name: String = r.get(1)?;
        if name == "seed" {
            has_seed = true;
            break;
        }
    }
    if !has_seed {
        conn.execute("ALTER TABLE benchmark_runs ADD COLUMN seed INTEGER", [])?;
    }

    let mut kstmt = conn.prepare("PRAGMA table_info(kg_facts)")?;
    let mut has_kg_bank = false;
    let mut krows = kstmt.query([])?;
    while let Some(r) = krows.next()? {
        let name: String = r.get(1)?;
        if name == "bank_id" {
            has_kg_bank = true;
            break;
        }
    }
    if !has_kg_bank {
        conn.execute(
            "ALTER TABLE kg_facts ADD COLUMN bank_id TEXT NOT NULL DEFAULT 'default'",
            [],
        )?;
    }
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_kg_bank_subject ON kg_facts(bank_id, subject)",
        [],
    )?;

    let mut tstmt = conn.prepare("PRAGMA table_info(tunnels)")?;
    let mut has_tunnel_bank = false;
    let mut trows = tstmt.query([])?;
    while let Some(r) = trows.next()? {
        let name: String = r.get(1)?;
        if name == "bank_id" {
            has_tunnel_bank = true;
            break;
        }
    }
    if !has_tunnel_bank {
        conn.execute(
            "ALTER TABLE tunnels ADD COLUMN bank_id TEXT NOT NULL DEFAULT 'default'",
            [],
        )?;
    }
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_tunnels_bank ON tunnels(bank_id)",
        [],
    )?;

    Ok(())
}

pub fn insert_tunnel(
    conn: &Connection,
    from_wing: &str,
    from_room: &str,
    to_wing: &str,
    to_room: &str,
    created_at: &str,
    bank_id: Option<&str>,
) -> Result<()> {
    let bank = bank_id.unwrap_or("default");
    conn.execute(
        "INSERT INTO tunnels(from_wing, from_room, to_wing, to_room, bank_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![from_wing, from_room, to_wing, to_room, bank, created_at],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_schema_migrates_old_bankless_tables_before_bank_indexes() {
        let conn = Connection::open_in_memory().expect("open memory db");
        conn.execute_batch(
            r#"
            CREATE TABLE drawers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                wing TEXT NOT NULL,
                hall TEXT NOT NULL,
                room TEXT NOT NULL,
                source_path TEXT NOT NULL,
                content TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE tunnels (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                from_wing TEXT NOT NULL,
                from_room TEXT NOT NULL,
                to_wing TEXT NOT NULL,
                to_room TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE kg_facts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                subject TEXT NOT NULL,
                predicate TEXT NOT NULL,
                object TEXT NOT NULL,
                valid_from TEXT NOT NULL,
                valid_to TEXT,
                source_drawer_id INTEGER,
                created_at TEXT NOT NULL
            );
            CREATE TABLE benchmark_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                mode TEXT NOT NULL,
                samples INTEGER NOT NULL,
                top_k INTEGER NOT NULL,
                recall REAL NOT NULL,
                latency_ms INTEGER NOT NULL,
                throughput_per_sec REAL NOT NULL,
                created_at TEXT NOT NULL
            );
            "#,
        )
        .expect("create old schema");

        init_schema(&conn).expect("migrate old schema");

        assert!(has_column(&conn, "drawers", "bank_id"));
        assert!(has_column(&conn, "tunnels", "bank_id"));
        assert!(has_column(&conn, "kg_facts", "bank_id"));
        conn.execute(
            "INSERT INTO tunnels(from_wing, from_room, to_wing, to_room, created_at)
             VALUES('w', 'a', 'w', 'b', 'now')",
            [],
        )
        .expect("legacy insert gets default bank");
        let bank: String = conn
            .query_row("SELECT bank_id FROM tunnels", [], |r| r.get(0))
            .expect("read tunnel bank");
        assert_eq!(bank, "default");
    }

    fn has_column(conn: &Connection, table: &str, column: &str) -> bool {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("pragma");
        let mut rows = stmt.query([]).expect("query columns");
        while let Some(row) = rows.next().expect("next column") {
            let name: String = row.get(1).expect("column name");
            if name == column {
                return true;
            }
        }
        false
    }
}
