use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

#[derive(Clone, Debug)]
pub struct SessionRecord {
    pub id: String,
    pub title: String,
    pub profile: String,
    pub updated_at: String,
}

#[derive(Clone, Debug)]
pub struct MessageRecord {
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug)]
pub struct SessionStore {
    path: PathBuf,
}

impl SessionStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let store = Self { path };
        store.with_conn(|conn| {
            conn.execute_batch(
                r#"
CREATE TABLE IF NOT EXISTS agent_sessions (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  profile TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS agent_messages (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  role TEXT NOT NULL,
  content TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY(session_id) REFERENCES agent_sessions(id)
);
"#,
            )?;
            Ok::<_, rusqlite::Error>(())
        })?;
        Ok(store)
    }

    pub fn ensure_session(
        &self,
        session_id: Option<String>,
        title_hint: &str,
        profile: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        if let Some(id) = session_id {
            if self.session_exists(&id)? {
                return Ok(id);
            }
            return Err(format!("unknown session: {id}").into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        let title = title_from_hint(title_hint);
        let now = now_string();
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO agent_sessions(id,title,profile,created_at,updated_at) VALUES (?1,?2,?3,?4,?5)",
                params![id, title, profile, now, now],
            )?;
            Ok::<_, rusqlite::Error>(())
        })?;
        Ok(id)
    }

    pub fn add_message(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let now = now_string();
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO agent_messages(session_id,role,content,created_at) VALUES (?1,?2,?3,?4)",
                params![session_id, role, content, now],
            )?;
            conn.execute(
                "UPDATE agent_sessions SET updated_at=?1 WHERE id=?2",
                params![now, session_id],
            )?;
            Ok::<_, rusqlite::Error>(())
        })?;
        Ok(())
    }

    pub fn list_sessions(&self) -> Result<Vec<SessionRecord>, Box<dyn std::error::Error>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id,title,profile,updated_at FROM agent_sessions ORDER BY updated_at DESC",
            )?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(SessionRecord {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        profile: row.get(2)?,
                        updated_at: row.get(3)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok::<_, rusqlite::Error>(rows)
        })
        .map_err(Into::into)
    }

    pub fn render_list(&self) -> Result<String, Box<dyn std::error::Error>> {
        let mut out = String::from("sessions\n");
        for session in self.list_sessions()? {
            out.push_str(&format!(
                "{}\t{}\t{}\t{}\n",
                session.id, session.profile, session.updated_at, session.title
            ));
        }
        Ok(out)
    }

    pub fn render_session(&self, session_id: &str) -> Result<String, Box<dyn std::error::Error>> {
        let mut out = format!("session {session_id}\n");
        for message in self.messages(session_id)? {
            out.push_str(&format!(
                "[{}] {}: {}\n",
                message.created_at, message.role, message.content
            ));
        }
        Ok(out)
    }

    pub fn messages(
        &self,
        session_id: &str,
    ) -> Result<Vec<MessageRecord>, Box<dyn std::error::Error>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT role,content,created_at FROM agent_messages WHERE session_id=?1 ORDER BY id ASC",
            )?;
            let rows = stmt
                .query_map(params![session_id], |row| {
                    Ok(MessageRecord {
                        role: row.get(0)?,
                        content: row.get(1)?,
                        created_at: row.get(2)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok::<_, rusqlite::Error>(rows)
        })
        .map_err(Into::into)
    }

    fn session_exists(&self, session_id: &str) -> Result<bool, Box<dyn std::error::Error>> {
        self.with_conn(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM agent_sessions WHERE id=?1",
                params![session_id],
                |row| row.get(0),
            )?;
            Ok::<_, rusqlite::Error>(count > 0)
        })
        .map_err(Into::into)
    }

    fn with_conn<T>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, rusqlite::Error>,
    ) -> Result<T, rusqlite::Error> {
        let conn = Connection::open(&self.path)?;
        f(&conn)
    }
}

fn title_from_hint(input: &str) -> String {
    let title = input.trim().chars().take(80).collect::<String>();
    if title.is_empty() {
        "chat session".to_string()
    } else {
        title
    }
}

fn now_string() -> String {
    OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}
