//! Local SQLite for request logs / token stats (layer B foundations).
//!
//! Path: `~/.grok-switch/grok-switch.db`. Provider/account JSON stores remain
//! the source of truth for configuration; this DB is for high-volume telemetry.

use crate::core::paths::Paths;
use crate::core::AppError;
use chrono::Local;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

static DB_INIT: Mutex<bool> = Mutex::new(false);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestLog {
    pub id: i64,
    pub ts: i64,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub method: String,
    pub path: String,
    pub status: u16,
    pub latency_ms: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenStats {
    pub requests: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub ok_count: u64,
    pub fail_count: u64,
}

pub fn open(paths: &Paths) -> Result<Connection, AppError> {
    paths.ensure_app_dirs()?;
    let conn = Connection::open(&paths.app_db)
        .map_err(|e| AppError::Message(format!("open db: {e}")))?;
    ensure_schema(&conn)?;
    Ok(conn)
}

fn ensure_schema(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS request_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL,
            provider_id TEXT,
            model TEXT,
            method TEXT NOT NULL,
            path TEXT NOT NULL,
            status INTEGER NOT NULL,
            latency_ms INTEGER NOT NULL,
            prompt_tokens INTEGER NOT NULL DEFAULT 0,
            completion_tokens INTEGER NOT NULL DEFAULT 0,
            ok INTEGER NOT NULL,
            detail TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_request_logs_ts ON request_logs(ts DESC);
        "#,
    )
    .map_err(|e| AppError::Message(format!("schema: {e}")))?;
    if let Ok(mut flag) = DB_INIT.lock() {
        *flag = true;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn insert_request_log(
    paths: &Paths,
    provider_id: Option<&str>,
    model: Option<&str>,
    method: &str,
    path: &str,
    status: u16,
    latency_ms: u64,
    prompt_tokens: u64,
    completion_tokens: u64,
    ok: bool,
    detail: &str,
) -> Result<i64, AppError> {
    let conn = open(paths)?;
    let ts = Local::now().timestamp();
    conn.execute(
        r#"INSERT INTO request_logs
           (ts, provider_id, model, method, path, status, latency_ms,
            prompt_tokens, completion_tokens, ok, detail)
           VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)"#,
        params![
            ts,
            provider_id,
            model,
            method,
            path,
            status as i64,
            latency_ms as i64,
            prompt_tokens as i64,
            completion_tokens as i64,
            if ok { 1 } else { 0 },
            detail,
        ],
    )
    .map_err(|e| AppError::Message(format!("insert log: {e}")))?;
    Ok(conn.last_insert_rowid())
}

pub fn list_request_logs(paths: &Paths, limit: usize) -> Result<Vec<RequestLog>, AppError> {
    let conn = open(paths)?;
    let limit = if limit == 0 { 50 } else { limit.min(500) };
    let mut stmt = conn
        .prepare(
            r#"SELECT id, ts, provider_id, model, method, path, status, latency_ms,
                      prompt_tokens, completion_tokens, ok, detail
               FROM request_logs ORDER BY id DESC LIMIT ?1"#,
        )
        .map_err(|e| AppError::Message(format!("prepare: {e}")))?;
    let rows = stmt
        .query_map(params![limit as i64], |row| {
            Ok(RequestLog {
                id: row.get(0)?,
                ts: row.get(1)?,
                provider_id: row.get(2)?,
                model: row.get(3)?,
                method: row.get(4)?,
                path: row.get(5)?,
                status: row.get::<_, i64>(6)? as u16,
                latency_ms: row.get::<_, i64>(7)? as u64,
                prompt_tokens: row.get::<_, i64>(8)? as u64,
                completion_tokens: row.get::<_, i64>(9)? as u64,
                ok: row.get::<_, i64>(10)? != 0,
                detail: row.get(11)?,
            })
        })
        .map_err(|e| AppError::Message(format!("query: {e}")))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| AppError::Message(format!("row: {e}")))?);
    }
    Ok(out)
}

pub fn token_stats(paths: &Paths) -> Result<TokenStats, AppError> {
    let conn = open(paths)?;
    let mut stmt = conn
        .prepare(
            r#"SELECT
                COUNT(*),
                COALESCE(SUM(prompt_tokens),0),
                COALESCE(SUM(completion_tokens),0),
                COALESCE(SUM(CASE WHEN ok=1 THEN 1 ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN ok=0 THEN 1 ELSE 0 END),0)
               FROM request_logs"#,
        )
        .map_err(|e| AppError::Message(format!("prepare stats: {e}")))?;
    let stats = stmt
        .query_row([], |row| {
            Ok(TokenStats {
                requests: row.get::<_, i64>(0)? as u64,
                prompt_tokens: row.get::<_, i64>(1)? as u64,
                completion_tokens: row.get::<_, i64>(2)? as u64,
                ok_count: row.get::<_, i64>(3)? as u64,
                fail_count: row.get::<_, i64>(4)? as u64,
            })
        })
        .map_err(|e| AppError::Message(format!("stats: {e}")))?;
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::paths::Paths;

    #[test]
    fn insert_list_and_stats() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_root(dir.path());
        paths.ensure_app_dirs().unwrap();

        insert_request_log(
            &paths,
            Some("p1"),
            Some("grok-4.5"),
            "POST",
            "/v1/chat/completions",
            200,
            42,
            10,
            20,
            true,
            "ok",
        )
        .unwrap();
        insert_request_log(
            &paths,
            Some("p1"),
            Some("grok-4.5"),
            "POST",
            "/v1/chat/completions",
            429,
            15,
            5,
            0,
            false,
            "rate limited",
        )
        .unwrap();

        let logs = list_request_logs(&paths, 10).unwrap();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].status, 429);

        let stats = token_stats(&paths).unwrap();
        assert_eq!(stats.requests, 2);
        assert_eq!(stats.prompt_tokens, 15);
        assert_eq!(stats.completion_tokens, 20);
        assert_eq!(stats.ok_count, 1);
        assert_eq!(stats.fail_count, 1);
    }
}
