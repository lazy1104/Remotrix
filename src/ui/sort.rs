use crate::message::{SortField, SortOrder};
use crate::task::DownloadTask;

pub fn sort_tasks<'a>(
    tasks: &[&'a DownloadTask],
    field: SortField,
    order: SortOrder,
) -> Vec<&'a DownloadTask> {
    let mut sorted = tasks.to_vec();
    sorted.sort_by(|a, b| {
        let cmp = match field {
            SortField::AddedTime => a.gid.cmp(&b.gid),
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

fn status_rank(status: crate::task::TaskStatus) -> u8 {
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
