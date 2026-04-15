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
