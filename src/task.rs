use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub gid: String,
    pub name: String,
    pub url: String,
    pub save_dir: PathBuf,
    pub downloaded: u64,
    pub total: u64,
    pub speed: u64,
    pub status: TaskStatus,
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
    pub fn label(self) -> &'static str {
        match self {
            TaskStatus::Waiting => "Waiting",
            TaskStatus::Active => "Downloading",
            TaskStatus::Paused => "Paused",
            TaskStatus::Completed => "Completed",
            TaskStatus::Error => "Error",
            TaskStatus::Removed => "Removed",
        }
    }
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
