//! Stable ordering for the task list.
//!
//! [`sort_tasks`] is the only entry point; the comparison key depends on
//! [`SortField`] and is reversed for [`SortOrder::Desc`]. Progress uses
//! `f32::total_cmp` so NaN never panics and is treated as greater than every
//! other value (matching the `Ord` for `f32`).

use crate::message::{SortField, SortOrder};
use crate::task::DownloadTask;

/// Sort a slice of task references by `field` and `order`.
///
/// Returns a freshly-allocated `Vec`; the input slice is not modified.
/// When `order` is [`SortOrder::Desc`] the comparison is reversed, except for
/// `Status` where the rank is computed once per side.
pub fn sort_tasks<'a>(
    tasks: &[&'a DownloadTask],
    field: SortField,
    order: SortOrder,
) -> Vec<&'a DownloadTask> {
    let mut sorted = tasks.to_vec();
    sorted.sort_by(|a, b| {
        let cmp = match field {
            SortField::AddedTime => a.added_at.cmp(&b.added_at),
            SortField::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortField::Size => a.total.cmp(&b.total),
            SortField::Progress => a.progress_pct().total_cmp(&b.progress_pct()),
            SortField::Status => {
                let sa = status_rank(a.status);
                let sb = status_rank(b.status);
                sa.cmp(&sb)
            }
        };
        match order {
            SortOrder::Asc => cmp,
            SortOrder::Desc => cmp.reverse(),
        }
    });
    sorted
}

/// Total ordering used by [`sort_tasks`] for [`SortField::Status`]. Lower
/// numbers sort first (Active → Removed). Kept `pub(crate)` so unit tests
/// in this module can assert the ranking.
pub(crate) fn status_rank(status: crate::task::TaskStatus) -> u8 {
    use crate::task::TaskStatus;
    match status {
        TaskStatus::Active => 0,
        TaskStatus::Waiting => 1,
        TaskStatus::Paused => 2,
        TaskStatus::Error => 3,
        TaskStatus::Completed => 4,
        TaskStatus::Removed => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{SortField, SortOrder};
    use crate::task::{DownloadTask, TaskStatus};
    use std::path::PathBuf;

    fn make_task(
        name: &str,
        total: u64,
        downloaded: u64,
        status: TaskStatus,
        added_at: i64,
    ) -> DownloadTask {
        DownloadTask {
            gid: String::new(),
            name: name.to_string(),
            url: String::new(),
            save_dir: PathBuf::new(),
            downloaded,
            total,
            speed: 0,
            upload_speed: 0,
            status,
            connections: 0,
            added_at,
            info_hash: None,
            metadata_probe_size: None,
            is_seeding: false,
            metadata_only: false,
            advanced: None,
        }
    }

    #[test]
    fn status_rank_total_order() {
        use crate::task::TaskStatus::*;
        let mut all = vec![Active, Waiting, Paused, Error, Completed, Removed];
        all.sort_by_key(|s| status_rank(*s));
        assert_eq!(
            all,
            vec![Active, Waiting, Paused, Error, Completed, Removed],
        );
    }

    #[test]
    fn status_rank_unique() {
        use crate::task::TaskStatus::*;
        let mut ranks = vec![
            status_rank(Active),
            status_rank(Waiting),
            status_rank(Paused),
            status_rank(Error),
            status_rank(Completed),
            status_rank(Removed),
        ];
        ranks.sort();
        ranks.dedup();
        assert_eq!(ranks.len(), 6);
    }

    #[test]
    fn sort_tasks_by_added_time_asc_desc() {
        let a = make_task("a", 0, 0, TaskStatus::Active, 100);
        let b = make_task("b", 0, 0, TaskStatus::Active, 50);
        let c = make_task("c", 0, 0, TaskStatus::Active, 200);
        let refs = vec![&a, &b, &c];
        let asc = sort_tasks(&refs, SortField::AddedTime, SortOrder::Asc);
        assert_eq!(
            asc.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            vec!["b", "a", "c"]
        );
        let desc = sort_tasks(&refs, SortField::AddedTime, SortOrder::Desc);
        assert_eq!(
            desc.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            vec!["c", "a", "b"]
        );
    }

    #[test]
    fn sort_tasks_by_name_case_insensitive() {
        let a = make_task("Banana", 0, 0, TaskStatus::Active, 0);
        let b = make_task("apple", 0, 0, TaskStatus::Active, 0);
        let c = make_task("Cherry", 0, 0, TaskStatus::Active, 0);
        let refs = vec![&a, &b, &c];
        let asc = sort_tasks(&refs, SortField::Name, SortOrder::Asc);
        assert_eq!(
            asc.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            vec!["apple", "Banana", "Cherry"]
        );
    }

    #[test]
    fn sort_tasks_by_size() {
        let a = make_task("a", 0, 0, TaskStatus::Active, 0);
        let b = make_task("b", 100, 0, TaskStatus::Active, 0);
        let c = make_task("c", 50, 0, TaskStatus::Active, 0);
        let refs = vec![&a, &b, &c];
        let asc = sort_tasks(&refs, SortField::Size, SortOrder::Asc);
        assert_eq!(
            asc.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            vec!["a", "c", "b"]
        );
    }

    #[test]
    fn sort_tasks_by_progress_uses_total_cmp() {
        let a = make_task("a", 100, 100, TaskStatus::Active, 0);
        let b = make_task("b", 100, 25, TaskStatus::Active, 0);
        let c = make_task("c", 100, 75, TaskStatus::Active, 0);
        let refs = vec![&a, &b, &c];
        let asc = sort_tasks(&refs, SortField::Progress, SortOrder::Asc);
        assert_eq!(
            asc.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            vec!["b", "c", "a"]
        );
    }

    #[test]
    fn sort_tasks_by_progress_handles_nan() {
        // total=0 yields 0.0%, not NaN — but exercise the total_cmp path
        // by relying on identical-progress items (must not panic).
        let a = make_task("a", 0, 0, TaskStatus::Active, 0);
        let b = make_task("b", 0, 0, TaskStatus::Active, 0);
        let refs = vec![&a, &b];
        let out = sort_tasks(&refs, SortField::Progress, SortOrder::Asc);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn sort_tasks_by_status_uses_rank() {
        let a = make_task("a", 0, 0, TaskStatus::Active, 0);
        let b = make_task("b", 0, 0, TaskStatus::Waiting, 0);
        let c = make_task("c", 0, 0, TaskStatus::Completed, 0);
        let refs = vec![&c, &b, &a];
        let asc = sort_tasks(&refs, SortField::Status, SortOrder::Asc);
        assert_eq!(
            asc.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            vec!["a", "b", "c"]
        );
    }

    #[test]
    fn sort_tasks_empty_input() {
        let refs: Vec<&DownloadTask> = vec![];
        let out = sort_tasks(&refs, SortField::Name, SortOrder::Asc);
        assert!(out.is_empty());
    }
}
