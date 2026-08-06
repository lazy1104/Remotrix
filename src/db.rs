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
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
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
        let advanced_cols = [
            "user_agent",
            "http_user",
            "http_passwd",
            "referer",
            "cookie",
            "proxy_server",
            "proxy_username",
            "proxy_password",
        ];
        for col in advanced_cols {
            let has: bool = conn
                .prepare("PRAGMA table_info(tasks)")
                .map_err(|e| format!("pragma: {e}"))?
                .query_map([], |row| row.get::<_, String>(1))
                .map_err(|e| format!("query map: {e}"))?
                .filter_map(|r| r.ok())
                .any(|c| c == col);
            if !has {
                conn.execute_batch(&format!(
                    "ALTER TABLE tasks ADD COLUMN {col} TEXT NOT NULL DEFAULT '';"
                ))
                .map_err(|e| format!("add column {col}: {e}"))?;
            }
        }
        Ok(Db {
            conn: std::sync::Mutex::new(conn),
        })
    }

    pub fn load_all(&self) -> Vec<DownloadTask> {
        let conn = self.conn.lock().expect("db lock");
        let mut stmt = match conn.prepare(
            "SELECT gid, name, url, dir, downloaded, total, speed, upload_speed, connections, status, added_at, info_hash,
                   user_agent, http_user, http_passwd, referer, cookie, proxy_server, proxy_username, proxy_password
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
            let advanced = crate::task::TaskAdvancedOptions {
                out: String::new(),
                user_agent: row.get(12)?,
                http_user: row.get(13)?,
                http_passwd: row.get(14)?,
                referer: row.get(15)?,
                cookie: row.get(16)?,
                proxy_server: row.get(17)?,
                proxy_username: row.get(18)?,
                proxy_password: row.get(19)?,
            };
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
                metadata_probe_size: None,
                is_seeding: false,
                advanced: if advanced.is_empty() {
                    None
                } else {
                    Some(advanced)
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
        advanced: Option<&crate::task::TaskAdvancedOptions>,
    ) {
        let conn = self.conn.lock().expect("db lock");
        let (
            user_agent,
            http_user,
            http_passwd,
            referer,
            cookie,
            proxy_server,
            proxy_username,
            proxy_password,
        ) = match advanced {
            Some(a) => (
                a.user_agent.as_str(),
                a.http_user.as_str(),
                a.http_passwd.as_str(),
                a.referer.as_str(),
                a.cookie.as_str(),
                a.proxy_server.as_str(),
                a.proxy_username.as_str(),
                a.proxy_password.as_str(),
            ),
            None => ("", "", "", "", "", "", "", ""),
        };
        if let Err(e) = conn.execute(
            "INSERT INTO tasks (gid, name, url, dir, downloaded, total, speed, connections, status, added_at, info_hash,
                                user_agent, http_user, http_passwd, referer, cookie, proxy_server, proxy_username, proxy_password)
             VALUES (?1, ?2, ?3, ?4, 0, 0, 0, 0, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(gid) DO UPDATE SET
                name=excluded.name, url=excluded.url, dir=excluded.dir, status=excluded.status, info_hash=excluded.info_hash,
                user_agent=excluded.user_agent, http_user=excluded.http_user, http_passwd=excluded.http_passwd,
                referer=excluded.referer, cookie=excluded.cookie, proxy_server=excluded.proxy_server,
                proxy_username=excluded.proxy_username, proxy_password=excluded.proxy_password",
            rusqlite::params![
                gid, name, url, dir, status, added_at, info_hash,
                user_agent, http_user, http_passwd, referer, cookie,
                proxy_server, proxy_username, proxy_password
            ],
        ) {
            tracing::error!(?gid, error = %e, "db upsert_meta failed");
        }
    }

    pub fn flush(&self, dirty: &[(String, u64, u64, u64, u64, u64, String)]) {
        let conn = self.conn.lock().expect("db lock");
        if let Err(e) = conn.execute_batch("BEGIN") {
            tracing::error!(error = %e, "db flush begin failed");
            return;
        }
        for (gid, downloaded, total, speed, upload_speed, connections, status) in dirty {
            if let Err(e) = conn.execute(
                "UPDATE tasks SET downloaded=?1, total=?2, speed=?3, upload_speed=?4, connections=?5, status=?6 WHERE gid=?7",
                rusqlite::params![downloaded, total, speed, upload_speed, connections, status, gid],
            ) {
                tracing::error!(?gid, error = %e, "db flush update failed");
            }
        }
        if let Err(e) = conn.execute_batch("COMMIT") {
            tracing::error!(error = %e, "db flush commit failed");
        }
    }

    pub fn update_name(&self, gid: &str, name: &str) {
        let conn = self.conn.lock().expect("db lock");
        if let Err(e) = conn.execute(
            "UPDATE tasks SET name=?1 WHERE gid=?2",
            rusqlite::params![name, gid],
        ) {
            tracing::error!(?gid, error = %e, "db update_name failed");
        }
    }

    pub fn delete(&self, gid: &str) {
        let conn = self.conn.lock().expect("db lock");
        if let Err(e) = conn.execute("DELETE FROM tasks WHERE gid=?1", rusqlite::params![gid]) {
            tracing::error!(?gid, error = %e, "db delete failed");
        }
    }

    pub fn delete_all(&self) {
        let conn = self.conn.lock().expect("db lock");
        if let Err(e) = conn.execute_batch("DELETE FROM tasks") {
            tracing::error!(error = %e, "db delete_all failed");
        }
    }

    pub fn clear_completed(&self, gids: &[String]) {
        let conn = self.conn.lock().expect("db lock");
        for gid in gids {
            if let Err(e) = conn.execute("DELETE FROM tasks WHERE gid=?1", rusqlite::params![gid]) {
                tracing::error!(?gid, error = %e, "db clear_completed failed");
            }
        }
    }
}
