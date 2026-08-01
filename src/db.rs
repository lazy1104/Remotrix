use std::path::Path;

use rusqlite::Connection;

use crate::task::{DownloadTask, TaskStatus};

pub struct Db {
    conn: std::sync::Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("create db dir: {e}"))?;
        }
        let conn = Connection::open(path).map_err(|e| format!("open db: {e}"))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA busy_timeout=5000;",
        )
        .map_err(|e| format!("db pragma: {e}"))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tasks (
                gid TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                url TEXT NOT NULL DEFAULT '',
                dir TEXT NOT NULL DEFAULT '',
                downloaded INTEGER NOT NULL DEFAULT 0,
                total INTEGER NOT NULL DEFAULT 0,
                speed INTEGER NOT NULL DEFAULT 0,
                connections INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL,
                added_at INTEGER NOT NULL
            );",
        )
        .map_err(|e| format!("create table: {e}"))?;

        let has_col: bool = conn
            .prepare("PRAGMA table_info(tasks)")
            .map_err(|e| format!("pragma: {e}"))?
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| format!("query map: {e}"))?
            .filter_map(|r| r.ok())
            .any(|c| c == "upload_speed");
        if !has_col {
            conn.execute_batch(
                "ALTER TABLE tasks ADD COLUMN upload_speed INTEGER NOT NULL DEFAULT 0;",
            )
            .map_err(|e| format!("add column: {e}"))?;
        }
        let has_info_hash: bool = conn
            .prepare("PRAGMA table_info(tasks)")
            .map_err(|e| format!("pragma: {e}"))?
            .query_map([], |row| {
                Ok((row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            })
            .map_err(|e| format!("query map: {e}"))?
            .filter_map(|r| r.ok())
            .any(|(c, t)| c == "info_hash" && t.to_lowercase().contains("text"));
        if !has_info_hash {
            conn.execute_batch("ALTER TABLE tasks ADD COLUMN info_hash TEXT NOT NULL DEFAULT '';")
                .map_err(|e| format!("add column: {e}"))?;
        }
        Ok(Db {
            conn: std::sync::Mutex::new(conn),
        })
    }

    pub fn load_all(&self) -> Vec<DownloadTask> {
        let conn = self.conn.lock().expect("db lock");
        let mut stmt = match conn.prepare(
            "SELECT gid, name, url, dir, downloaded, total, speed, upload_speed, connections, status, added_at, info_hash
             FROM tasks ORDER BY added_at DESC",
        ) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("load_all: prepare failed: {e}");
                return Vec::new();
            }
        };
        let rows = match stmt.query_map([], |row| {
            let status_str: String = row.get(9)?;
            let status = match status_str.as_str() {
                "waiting" => TaskStatus::Waiting,
                "active" => TaskStatus::Active,
                "paused" => TaskStatus::Paused,
                "complete" => TaskStatus::Completed,
                "error" => TaskStatus::Error,
                "removed" => TaskStatus::Removed,
                _ => TaskStatus::Waiting,
            };
            let info_hash: String = row.get(11)?;
            Ok(DownloadTask {
                gid: row.get(0)?,
                name: row.get(1)?,
                url: row.get(2)?,
                save_dir: row.get::<_, String>(3)?.into(),
                downloaded: row.get(4)?,
                total: row.get(5)?,
                speed: row.get(6)?,
                upload_speed: row.get(7)?,
                connections: row.get(8)?,
                status,
                added_at: row.get(10)?,
                info_hash: if info_hash.is_empty() {
                    None
                } else {
                    Some(info_hash)
                },
            })
        }) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("load_all: query failed: {e}");
                return Vec::new();
            }
        };
        rows.filter_map(|r| match r {
            Ok(t) => Some(t),
            Err(e) => {
                tracing::error!("load_all: row decode failed: {e}");
                None
            }
        })
        .collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn upsert_meta(
        &self,
        gid: &str,
        name: &str,
        url: &str,
        dir: &str,
        status: &str,
        added_at: i64,
        info_hash: &str,
    ) {
        let conn = self.conn.lock().expect("db lock");
        let _ = conn.execute(
            "INSERT INTO tasks (gid, name, url, dir, downloaded, total, speed, connections, status, added_at, info_hash)
             VALUES (?1, ?2, ?3, ?4, 0, 0, 0, 0, ?5, ?6, ?7)
             ON CONFLICT(gid) DO UPDATE SET
                name=excluded.name, url=excluded.url, dir=excluded.dir, status=excluded.status, info_hash=excluded.info_hash",
            rusqlite::params![gid, name, url, dir, status, added_at, info_hash],
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn upsert_progress(
        &self,
        gid: &str,
        downloaded: u64,
        total: u64,
        speed: u64,
        upload_speed: u64,
        connections: u64,
        status: &str,
    ) {
        let conn = self.conn.lock().expect("db lock");
        let _ = conn.execute(
            "UPDATE tasks SET downloaded=?1, total=?2, speed=?3, upload_speed=?4, connections=?5, status=?6 WHERE gid=?7",
            rusqlite::params![downloaded, total, speed, upload_speed, connections, status, gid],
        );
    }

    pub fn flush(&self, dirty: &[(String, u64, u64, u64, u64, u64, String)]) {
        let conn = self.conn.lock().expect("db lock");
        let _ = conn.execute_batch("BEGIN");
        for (gid, downloaded, total, speed, upload_speed, connections, status) in dirty {
            let _ = conn.execute(
                "UPDATE tasks SET downloaded=?1, total=?2, speed=?3, upload_speed=?4, connections=?5, status=?6 WHERE gid=?7",
                rusqlite::params![downloaded, total, speed, upload_speed, connections, status, gid],
            );
        }
        let _ = conn.execute_batch("COMMIT");
    }

    pub fn delete(&self, gid: &str) {
        let conn = self.conn.lock().expect("db lock");
        let _ = conn.execute("DELETE FROM tasks WHERE gid=?1", rusqlite::params![gid]);
    }

    pub fn delete_all(&self) {
        let conn = self.conn.lock().expect("db lock");
        let _ = conn.execute_batch("DELETE FROM tasks");
    }

    pub fn clear_completed(&self, gids: &[String]) {
        let conn = self.conn.lock().expect("db lock");
        for gid in gids {
            let _ = conn.execute("DELETE FROM tasks WHERE gid=?1", rusqlite::params![gid]);
        }
    }
}
