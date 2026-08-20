use std::path::PathBuf;

use aria2_ws::TaskOptions;

/// Per-task overrides applied on top of the global `aria2` options when a
/// task is first added.
///
/// Empty strings mean "fall back to the global default"; non-empty values are
/// written into the corresponding aria2 option. Used by the Add dialog and
/// when re-issuing a task from history.
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
    /// Apply this override set onto `opts` for a brand-new download.
    ///
    /// Only writes keys whose override string is non-empty, leaving every
    /// other option untouched. Empty strings signal "no override" so the
    /// underlying aria2/global defaults apply.
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

    /// Apply this override set onto `opts` for an existing task being edited.
    ///
    /// Unlike [`Self::apply`], every field is rewritten — including clearing
    /// previously-set headers by overwriting them with the empty string and
    /// resetting `all_proxy` to its default. Use when re-issuing an aria2
    /// task with a fresh option bag.
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

    /// Returns `true` when every override field is empty (i.e. this task uses
    /// only the global defaults).
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

/// Snapshot of a download tracked by the UI.
///
/// Mirrors the aria2 fields we surface to the user, plus a few UI-only flags
/// (`is_seeding`, `metadata_only`) that derive from aria2 events. All numeric
/// progress fields are monotonically non-decreasing per task.
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
    /// Returns the download progress as a percentage in `[0, 100]`.
    ///
    /// Returns `0.0` when `total` is `0` (unknown size) to avoid division by
    /// zero; otherwise clamps to `100.0` so overshoot from late-arriving
    /// `completed` updates never displays as more than complete.
    pub fn progress_pct(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.downloaded as f64 / self.total as f64 * 100.0).min(100.0) as f32
        }
    }

    /// Returns the estimated seconds remaining, or `None` when unknown.
    ///
    /// `None` is returned both when `speed == 0` (stalled/idle) and when the
    /// task has no remaining bytes (already complete). Saturating subtraction
    /// guards against overshoot from late progress updates.
    pub fn eta_secs(&self) -> Option<u64> {
        if self.speed == 0 {
            return None;
        }
        let remaining = self.total.saturating_sub(self.downloaded);
        Some(remaining / self.speed)
    }

    /// Returns `true` while the task is in a non-terminal aria2 state.
    pub fn is_download_active(&self) -> bool {
        matches!(self.status, TaskStatus::Active | TaskStatus::Waiting)
    }

    /// Returns `true` only when the task has reached the `Completed` state.
    /// For seeding BitTorrent tasks that remain `active`, use
    /// [`Self::is_download_complete`] instead.
    pub fn is_completed(&self) -> bool {
        matches!(self.status, TaskStatus::Completed)
    }

    /// Returns `true` when the user-visible download has finished, even if
    /// the task is still seeding. Seeding BitTorrent tasks stay `active` in
    /// aria2 but should be rendered as complete by the UI.
    pub fn is_download_complete(&self) -> bool {
        is_download_complete(self.status.to_str(), self.is_seeding)
    }
}

/// Underlying rule for [`DownloadTask::is_download_complete`].
///
/// A task is considered complete when its aria2 status is `complete`, or
/// when it is an `active` BitTorrent task that has switched to seeding mode
/// (aria2 reports `active` forever for seeders).
pub(crate) fn is_download_complete(status: &str, is_seeding: bool) -> bool {
    status == "complete" || (status == "active" && is_seeding)
}

/// High-level lifecycle states used by the UI to render and filter tasks.
///
/// These are a closed enum, not raw aria2 status strings, so the rest of the
/// app can pattern-match without worrying about aria2 introducing new state
/// names. Use [`Self::from_engine`] to bridge aria2's lowercase strings.
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
    /// Map an aria2 status string (e.g. `"active"`, `"complete"`) into a
    /// [`TaskStatus`]. Unrecognized values fall back to [`Self::Waiting`].
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

    /// The aria2 wire-format name for this state.
    ///
    /// This is the inverse of [`Self::from_engine`] and is used when sending
    /// `tell_status` queries back to aria2 or persisting tasks to disk.
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

/// Single file within a multi-file task (BitTorrent / metalink).
#[derive(Debug, Clone)]
pub struct TaskFile {
    pub index: u64,
    pub path: String,
    pub length: u64,
    pub completed_length: u64,
    pub selected: bool,
}

/// Extended metadata fetched lazily from aria2 for the details panel.
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

/// Render a byte count as a human-readable string with a binary (1024-based)
/// unit suffix.
///
/// `0` formats as `"0 B"` and values up to `1023` keep the integer `B` unit.
/// Larger values use one decimal place (`"1.5 KB"`). The largest unit is
/// `TB`; values larger than that are still reported in `TB`.
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

/// Render a transfer rate (bytes/second) as `"<size>/s"` using
/// [`format_size`] for the numeric portion.
pub fn format_speed(bytes_per_sec: u64) -> String {
    format!("{}/s", format_size(bytes_per_sec))
}

/// Render a duration in seconds as `MM:SS` or, once it crosses one hour, as
/// `HH:MM:SS`. Values are always zero-padded to two digits per segment.
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

/// Render a Unix timestamp (seconds) as the local-time `YYYY-MM-DD HH:MM:SS`
/// string used in task lists. Out-of-range timestamps render as `"1970-01-01
/// 00:00:00"` rather than panicking, since `chrono::DateTime::from_timestamp`
/// returns `None` for values outside its representable range.
pub fn format_add_time(unix_secs: i64) -> String {
    use chrono::{DateTime, Local, Utc};
    let dt: DateTime<Utc> = DateTime::from_timestamp(unix_secs, 0).unwrap_or_default();
    let local: DateTime<Local> = dt.with_timezone(&Local);
    local.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Count completed pieces from an aria2 hex-encoded bitfield.
///
/// Returns `(done, total)` where `total` is always `num_pieces`. Returns
/// `(0, num_pieces)` when the bitfield is `None`, empty, malformed hex, or
/// shorter than expected — never panics on bad input. Bits beyond
/// `num_pieces` in the input are ignored (aria2 sometimes pads with zeroes).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_size_bytes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1), "1 B");
        assert_eq!(format_size(1023), "1023 B");
    }

    #[test]
    fn format_size_kb_boundary() {
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1024 * 1024 - 1), "1024.0 KB");
    }

    #[test]
    fn format_size_mb_gb_tb() {
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(1024u64.pow(3)), "1.0 GB");
        assert_eq!(format_size(1024u64.pow(4)), "1.0 TB");
    }

    #[test]
    fn format_size_max_bytes() {
        let s = format_size(u64::MAX);
        assert!(s.contains("TB"));
    }

    #[test]
    fn format_speed_appends_per_second() {
        assert_eq!(format_speed(0), "0 B/s");
        assert_eq!(format_speed(1024), "1.0 KB/s");
        assert_eq!(format_speed(2 * 1024 * 1024), "2.0 MB/s");
    }

    #[test]
    fn format_duration_sub_minute() {
        assert_eq!(format_duration(0), "00:00");
        assert_eq!(format_duration(59), "00:59");
    }

    #[test]
    fn format_duration_under_hour() {
        assert_eq!(format_duration(60), "01:00");
        assert_eq!(format_duration(125), "02:05");
        assert_eq!(format_duration(3599), "59:59");
    }

    #[test]
    fn format_duration_hour_or_more() {
        assert_eq!(format_duration(3600), "01:00:00");
        assert_eq!(format_duration(3661), "01:01:01");
        assert_eq!(format_duration(86_400), "24:00:00");
    }

    #[test]
    fn format_add_time_returns_prefix() {
        let s = format_add_time(1_700_000_000);
        assert!(s.len() >= 10, "expected non-empty formatted time");
        assert!(
            s.starts_with("20") || s.starts_with("19"),
            "unexpected prefix: {s}"
        );
        assert!(s.as_bytes().get(4) == Some(&b'-'));
    }

    #[test]
    fn completed_pieces_none() {
        assert_eq!(completed_pieces(None, 16), (0, 16));
    }

    #[test]
    fn completed_pieces_empty_bitfield() {
        assert_eq!(completed_pieces(Some(""), 16), (0, 16));
    }

    #[test]
    fn completed_pieces_zero_total() {
        assert_eq!(completed_pieces(Some("ffff"), 0), (0, 0));
    }

    #[test]
    fn completed_pieces_all_done() {
        assert_eq!(completed_pieces(Some("ffff"), 16), (16, 16));
    }

    #[test]
    fn completed_pieces_partial() {
        assert_eq!(completed_pieces(Some("ff00"), 16), (8, 16));
    }

    #[test]
    fn completed_pieces_short_bitfield() {
        assert_eq!(completed_pieces(Some("0f"), 16), (4, 16));
    }

    #[test]
    fn completed_pieces_invalid_hex() {
        assert_eq!(completed_pieces(Some("zzzz"), 8), (0, 8));
    }

    #[test]
    fn completed_pieces_ignores_padding() {
        assert_eq!(completed_pieces(Some("fffffff0"), 4), (4, 4));
    }

    #[test]
    fn is_download_complete_true_for_complete() {
        assert!(is_download_complete("complete", false));
        assert!(is_download_complete("complete", true));
    }

    #[test]
    fn is_download_complete_seeding_active() {
        assert!(is_download_complete("active", true));
        assert!(!is_download_complete("active", false));
    }

    #[test]
    fn is_download_complete_other_statuses() {
        assert!(!is_download_complete("waiting", true));
        assert!(!is_download_complete("paused", false));
        assert!(!is_download_complete("error", false));
        assert!(!is_download_complete("removed", false));
    }

    #[test]
    fn task_status_from_engine_known() {
        assert_eq!(TaskStatus::from_engine("waiting"), TaskStatus::Waiting);
        assert_eq!(TaskStatus::from_engine("active"), TaskStatus::Active);
        assert_eq!(TaskStatus::from_engine("paused"), TaskStatus::Paused);
        assert_eq!(TaskStatus::from_engine("complete"), TaskStatus::Completed);
        assert_eq!(TaskStatus::from_engine("error"), TaskStatus::Error);
        assert_eq!(TaskStatus::from_engine("removed"), TaskStatus::Removed);
    }

    #[test]
    fn task_status_from_engine_unknown_falls_back() {
        assert_eq!(TaskStatus::from_engine(""), TaskStatus::Waiting);
        assert_eq!(TaskStatus::from_engine("garbage"), TaskStatus::Waiting);
        assert_eq!(TaskStatus::from_engine("Active"), TaskStatus::Waiting);
    }

    #[test]
    fn task_status_to_str_round_trip() {
        for s in [
            TaskStatus::Waiting,
            TaskStatus::Active,
            TaskStatus::Paused,
            TaskStatus::Completed,
            TaskStatus::Error,
            TaskStatus::Removed,
        ] {
            assert_eq!(TaskStatus::from_engine(s.to_str()), s);
        }
    }

    fn blank_task(status: TaskStatus) -> DownloadTask {
        DownloadTask {
            gid: String::new(),
            name: String::new(),
            url: String::new(),
            save_dir: PathBuf::new(),
            downloaded: 0,
            total: 0,
            speed: 0,
            upload_speed: 0,
            status,
            connections: 0,
            added_at: 0,
            info_hash: None,
            metadata_probe_size: None,
            is_seeding: false,
            metadata_only: false,
            advanced: None,
        }
    }

    #[test]
    fn download_task_progress_pct() {
        let mut t = blank_task(TaskStatus::Active);
        assert_eq!(t.progress_pct(), 0.0);
        t.total = 100;
        t.downloaded = 50;
        assert!((t.progress_pct() - 50.0).abs() < 1e-3);
        t.downloaded = 200;
        assert_eq!(t.progress_pct(), 100.0);
    }

    #[test]
    fn download_task_eta() {
        let mut t = blank_task(TaskStatus::Active);
        assert_eq!(t.eta_secs(), None);
        t.speed = 100;
        t.total = 1000;
        t.downloaded = 200;
        assert_eq!(t.eta_secs(), Some(8));
        t.downloaded = t.total;
        assert_eq!(t.eta_secs(), Some(0));
    }

    #[test]
    fn download_task_active_and_completed() {
        let mut t = blank_task(TaskStatus::Waiting);
        assert!(t.is_download_active());
        assert!(!t.is_completed());
        assert!(!t.is_download_complete());
        t.status = TaskStatus::Active;
        t.is_seeding = true;
        assert!(t.is_download_active());
        assert!(t.is_download_complete());
        assert!(!t.is_completed());
        t.status = TaskStatus::Completed;
        assert!(t.is_completed());
        assert!(!t.is_download_active());
    }

    #[test]
    fn advanced_options_apply_only_non_empty() {
        let mut opts = TaskOptions::default();
        let adv = TaskAdvancedOptions {
            out: "name.bin".to_string(),
            user_agent: "ua".to_string(),
            ..Default::default()
        };
        adv.apply(&mut opts);
        assert_eq!(opts.out.as_deref(), Some("name.bin"));
        assert_eq!(
            opts.extra_options
                .get("user-agent")
                .and_then(|v| v.as_str()),
            Some("ua"),
        );
        assert!(opts.extra_options.get("http-user").is_none());
    }

    #[test]
    fn advanced_options_apply_change_always_writes() {
        let mut opts = TaskOptions::default();
        opts.extra_options.insert(
            "user-agent".to_string(),
            serde_json::Value::String("old".to_string()),
        );
        let adv = TaskAdvancedOptions::default();
        adv.apply_change(&mut opts);
        assert_eq!(
            opts.extra_options
                .get("user-agent")
                .and_then(|v| v.as_str()),
            Some(""),
        );
    }

    #[test]
    fn advanced_options_is_empty() {
        assert!(TaskAdvancedOptions::default().is_empty());
        let a = TaskAdvancedOptions {
            out: "x".to_string(),
            ..TaskAdvancedOptions::default()
        };
        assert!(!a.is_empty());
    }
}
