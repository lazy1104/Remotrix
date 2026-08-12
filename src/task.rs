use std::path::PathBuf;

use aria2_ws::TaskOptions;

#[derive(Debug, Clone, Default)]
pub struct TaskAdvancedOptions {
    pub out: String,
    pub user_agent: String,
    pub http_user: String,
    pub http_passwd: String,
    pub referer: String,
    pub cookie: String,
    pub proxy_server: String,
    pub proxy_username: String,
    pub proxy_password: String,
}

impl TaskAdvancedOptions {
    pub fn apply(&self, opts: &mut TaskOptions) {
        if !self.out.is_empty() {
            opts.out = Some(self.out.clone());
        }
        let extra = [
            ("user-agent", &self.user_agent),
            ("http-user", &self.http_user),
            ("http-passwd", &self.http_passwd),
            ("referer", &self.referer),
            ("cookie", &self.cookie),
        ];
        for (key, value) in extra {
            if !value.is_empty() {
                opts.extra_options
                    .insert(key.to_string(), serde_json::Value::String(value.clone()));
            }
        }
        if let Some(proxy) = crate::config::all_proxy_url(
            &self.proxy_server,
            &self.proxy_username,
            &self.proxy_password,
        ) {
            opts.all_proxy = Some(proxy);
        }
    }

    pub fn apply_change(&self, opts: &mut TaskOptions) {
        if !self.out.is_empty() {
            opts.out = Some(self.out.clone());
        }
        let extra = [
            ("user-agent", &self.user_agent),
            ("http-user", &self.http_user),
            ("http-passwd", &self.http_passwd),
            ("referer", &self.referer),
            ("cookie", &self.cookie),
        ];
        for (key, value) in extra {
            opts.extra_options
                .insert(key.to_string(), serde_json::Value::String(value.clone()));
        }
        opts.all_proxy = Some(
            crate::config::all_proxy_url(
                &self.proxy_server,
                &self.proxy_username,
                &self.proxy_password,
            )
            .unwrap_or_default(),
        );
    }

    pub fn is_empty(&self) -> bool {
        self.out.is_empty()
            && self.user_agent.is_empty()
            && self.http_user.is_empty()
            && self.http_passwd.is_empty()
            && self.referer.is_empty()
            && self.cookie.is_empty()
            && self.proxy_server.is_empty()
            && self.proxy_username.is_empty()
            && self.proxy_password.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub gid: String,
    pub name: String,
    pub url: String,
    pub save_dir: PathBuf,
    pub downloaded: u64,
    pub total: u64,
    pub speed: u64,
    pub upload_speed: u64,
    pub status: TaskStatus,
    pub connections: u64,
    pub added_at: i64,
    pub info_hash: Option<String>,
    pub metadata_probe_size: Option<u64>,
    pub is_seeding: bool,
    pub metadata_only: bool,
    pub advanced: Option<TaskAdvancedOptions>,
}

impl DownloadTask {
    pub fn progress_pct(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.downloaded as f64 / self.total as f64 * 100.0).min(100.0) as f32
        }
    }

    pub fn eta_secs(&self) -> Option<u64> {
        if self.speed == 0 {
            return None;
        }
        let remaining = self.total.saturating_sub(self.downloaded);
        Some(remaining / self.speed)
    }

    pub fn is_download_active(&self) -> bool {
        matches!(self.status, TaskStatus::Active | TaskStatus::Waiting)
    }

    pub fn is_completed(&self) -> bool {
        matches!(self.status, TaskStatus::Completed)
    }

    pub fn is_download_complete(&self) -> bool {
        is_download_complete(self.status.to_str(), self.is_seeding)
    }
}

pub(crate) fn is_download_complete(status: &str, is_seeding: bool) -> bool {
    status == "complete" || (status == "active" && is_seeding)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Waiting,
    Active,
    Paused,
    Completed,
    Error,
    Removed,
}

impl TaskStatus {
    pub fn from_engine(status: &str) -> Self {
        match status {
            "waiting" => TaskStatus::Waiting,
            "active" => TaskStatus::Active,
            "paused" => TaskStatus::Paused,
            "complete" => TaskStatus::Completed,
            "error" => TaskStatus::Error,
            "removed" => TaskStatus::Removed,
            _ => TaskStatus::Waiting,
        }
    }

    pub fn to_str(self) -> &'static str {
        match self {
            TaskStatus::Waiting => "waiting",
            TaskStatus::Active => "active",
            TaskStatus::Paused => "paused",
            TaskStatus::Completed => "complete",
            TaskStatus::Error => "error",
            TaskStatus::Removed => "removed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TaskFile {
    pub index: u64,
    pub path: String,
    pub length: u64,
    pub completed_length: u64,
    pub selected: bool,
}

#[derive(Debug, Clone)]
pub struct TaskDetails {
    pub bitfield: Option<String>,
    pub num_pieces: u64,
    pub piece_length: u64,
    pub files: Vec<TaskFile>,
    pub creation_date: Option<i64>,
    pub comment: Option<String>,
    pub mode: Option<String>,
}

pub fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.1} {}", value, UNITS[unit])
    }
}

pub fn format_speed(bytes_per_sec: u64) -> String {
    format!("{}/s", format_size(bytes_per_sec))
}

pub fn format_duration(secs: u64) -> String {
    if secs >= 3600 {
        format!(
            "{:02}:{:02}:{:02}",
            secs / 3600,
            (secs % 3600) / 60,
            secs % 60
        )
    } else {
        format!("{:02}:{:02}", secs / 60, secs % 60)
    }
}

pub fn format_add_time(unix_secs: i64) -> String {
    use chrono::{DateTime, Local, Utc};
    let dt: DateTime<Utc> = DateTime::from_timestamp(unix_secs, 0).unwrap_or_default();
    let local: DateTime<Local> = dt.with_timezone(&Local);
    local.format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn completed_pieces(bitfield: Option<&str>, num_pieces: u64) -> (u64, u64) {
    let Some(bf) = bitfield else {
        return (0, num_pieces);
    };
    if bf.is_empty() || num_pieces == 0 {
        return (0, num_pieces);
    }
    let bits = match hex::decode(bf) {
        Ok(b) => b,
        Err(_) => return (0, num_pieces),
    };
    let mut done = 0u64;
    let mut total_bits = 0u64;
    for &byte in &bits {
        for i in (0..8).rev() {
            if total_bits >= num_pieces {
                break;
            }
            if (byte >> i) & 1 == 1 {
                done += 1;
            }
            total_bits += 1;
        }
        if total_bits >= num_pieces {
            break;
        }
    }
    (done, num_pieces)
}
