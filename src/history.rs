use rusqlite::{Connection, params};
use serde::Serialize;
use std::sync::{Mutex, OnceLock};

use crate::cef_host;

static DATABASE: OnceLock<Mutex<Option<Connection>>> = OnceLock::new();

#[derive(Debug, Serialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub visited_at: i64,
    pub visit_count: i64,
}

fn database() -> &'static Mutex<Option<Connection>> {
    DATABASE.get_or_init(|| {
        let path = cef_host::cache_path().join("history.db");
        let connection = (|| {
            std::fs::create_dir_all(path.parent()?) .ok()?;
            let connection = Connection::open(path).ok()?;
            connection
                .execute_batch(
                    "PRAGMA journal_mode=WAL;
                     CREATE TABLE IF NOT EXISTS history (
                       id INTEGER PRIMARY KEY AUTOINCREMENT,
                       url TEXT NOT NULL UNIQUE,
                       title TEXT NOT NULL DEFAULT '',
                       visited_at INTEGER NOT NULL,
                       visit_count INTEGER NOT NULL DEFAULT 1
                     );
                     CREATE INDEX IF NOT EXISTS history_visited_at
                       ON history(visited_at DESC);",
                )
                .ok()?;
            Some(connection)
        })();
        Mutex::new(connection)
    })
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

pub fn record_visit(url: &str) {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return;
    }
    if let Ok(mut guard) = database().lock()
        && let Some(connection) = guard.as_mut()
    {
        let _ = connection.execute(
            "INSERT INTO history (url, visited_at) VALUES (?1, ?2)
             ON CONFLICT(url) DO UPDATE SET
               visited_at = excluded.visited_at,
               visit_count = history.visit_count + 1",
            params![url, now_unix()],
        );
    }
}

pub fn update_title(url: &str, title: &str) {
    if title.trim().is_empty() {
        return;
    }
    if let Ok(mut guard) = database().lock()
        && let Some(connection) = guard.as_mut()
    {
        let _ = connection.execute(
            "UPDATE history SET title = ?2 WHERE url = ?1",
            params![url, title],
        );
    }
}

pub fn list(query: &str, limit: usize) -> Result<Vec<HistoryEntry>, String> {
    let guard = database().lock().map_err(|_| "history lock poisoned")?;
    let connection = guard.as_ref().ok_or("history database unavailable")?;
    let pattern = format!("%{}%", query.trim());
    let mut statement = connection
        .prepare(
            "SELECT id, url, title, visited_at, visit_count FROM history
             WHERE ?1 = '%%' OR url LIKE ?1 OR title LIKE ?1
             ORDER BY visited_at DESC LIMIT ?2",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![pattern, limit.min(2000) as i64], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                visited_at: row.get(3)?,
                visit_count: row.get(4)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub fn remove(id: i64) -> Result<bool, String> {
    let mut guard = database().lock().map_err(|_| "history lock poisoned")?;
    let connection = guard.as_mut().ok_or("history database unavailable")?;
    connection
        .execute("DELETE FROM history WHERE id = ?1", params![id])
        .map(|changed| changed > 0)
        .map_err(|error| error.to_string())
}

pub fn clear() -> Result<(), String> {
    let mut guard = database().lock().map_err(|_| "history lock poisoned")?;
    let connection = guard.as_mut().ok_or("history database unavailable")?;
    connection
        .execute("DELETE FROM history", [])
        .map(|_| ())
        .map_err(|error| error.to_string())
}
