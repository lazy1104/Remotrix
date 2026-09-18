//! Sidebar that lists either the task list filters (when the active page is
//! `Tasks`) or the settings categories (when the active page is `Settings`).
//!
//! The widget owns no state — selection lives in [`App`](crate::app) and is
//! passed in via `task_filter` / `settings_cat`. The animated pill behind
//! the active item is driven by `pill` (an [`Animated<f32>`]) so the caller
//! can re-target it when the active item changes.

use iced::widget::{button, column, container, row, text, Stack};
use iced::{Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{Message, NavMsg, Page, SettingsCategory, TaskFilter};
use crate::ui::animation::{animation, Animated};
use crate::ui::components::translate::translate;
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

/// Per-filter counts displayed next to each sidebar entry.
///
/// All three fields are populated by the app each time the task list
/// changes; missing entries are simply rendered without a count.
pub struct Counts {
    pub all: usize,
    pub downloading: usize,
    pub completed: usize,
}

/// Position of `f` in the on-screen task filter list. Useful for animating
/// the pill to the right row.
pub fn task_filter_index(f: TaskFilter) -> usize {
    match f {
        TaskFilter::All => 0,
        TaskFilter::Downloading => 1,
        TaskFilter::Completed => 2,
    }
}

/// Position of `c` in the on-screen settings category list.
pub fn settings_cat_index(c: SettingsCategory) -> usize {
    match c {
        SettingsCategory::General => 0,
        SettingsCategory::Download => 1,
        SettingsCategory::BitTorrent => 2,
        SettingsCategory::Ed2k => 3,
        SettingsCategory::Network => 4,
        SettingsCategory::Advanced => 5,
    }
}

/// Background shell painted with `category_background` style. Sized to
/// fill whatever bounds the caller provides; contents are empty so it
/// renders as a flat coloured panel. Kept separate from [`content`] so
/// callers (e.g. `app.rs`) can wrap the content in a scale animation
/// while the background stays at 100%.
pub fn background<'a>(_theme: &iced::Theme) -> Element<'a, Message> {
    container(iced::widget::Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(PADDING_CATEGORY_BAR)
        .style(theme::style::category_background)
        .into()
}

/// The inner column (title + items + pill indicator) without the
/// background panel. Sized to fill the caller's bounds; carries the
/// same padding as [`background`] so layout is identical.
pub fn content<'a>(
    fluent: &'a Fluent,
    _theme: &iced::Theme,
    page: Page,
    task_filter: TaskFilter,
    settings_cat: SettingsCategory,
    counts: &Counts,
    pill: &'a Animated<f32>,
) -> Element<'a, Message> {
    let title_str = match page {
        Page::Tasks => fluent.get(Tr::TasksList),
        Page::Settings => fluent.get(Tr::Preferences),
    };

    let title = text(title_str).size(FONT_TITLE).font(iced::Font {
        weight: iced::font::Weight::Bold,
        ..Default::default()
    });

    let items: Element<'a, Message> = match page {
        Page::Tasks => {
            let make_filter =
                |label: String, count: usize, target: TaskFilter| -> Element<'a, Message> {
                    let is_active = task_filter == target;
                    let label_text: String = if count > 0 {
                        format!("{} ({})", label, count)
                    } else {
                        label
                    };
                    let icon = match target {
                        TaskFilter::All => icon::layers(),
                        TaskFilter::Downloading => icon::download_arrow(),
                        TaskFilter::Completed => icon::circle_check(),
                    };
                    button(
                        row![]
                            .push(icon.size(FONT_ICON).line_height(1.0))
                            .push(text(label_text).size(FONT_BODY).line_height(1.0))
                            .push(iced::widget::Space::new().width(Length::Fill))
                            .spacing(SPACE_LG)
                            .align_y(Alignment::Center)
                            .width(Length::Fill),
                    )
                    .on_press(Message::Nav(NavMsg::SetTaskFilter(target)))
                    .padding(PADDING_FILTER)
                    .height(Length::Fixed(FILTER_ITEM_H))
                    .width(Length::Fill)
                    .style(theme::style::button::filter(is_active))
                    .into()
                };

            column![]
                .spacing(SPACE_MD)
                .push(make_filter(
                    fluent.get(Tr::All),
                    counts.all,
                    TaskFilter::All,
                ))
                .push(make_filter(
                    fluent.get(Tr::Downloading),
                    counts.downloading,
                    TaskFilter::Downloading,
                ))
                .push(make_filter(
                    fluent.get(Tr::Completed),
                    counts.completed,
                    TaskFilter::Completed,
                ))
                .into()
        }
        Page::Settings => {
            let make_cat = |label: String, target: SettingsCategory| -> Element<'a, Message> {
                let is_active = settings_cat == target;
                let icon = match target {
                    SettingsCategory::General => icon::sliders(),
                    SettingsCategory::Download => icon::download(),
                    SettingsCategory::BitTorrent => icon::magnet(),
                    SettingsCategory::Ed2k => icon::share(),
                    SettingsCategory::Network => icon::globe(),
                    SettingsCategory::Advanced => icon::wrench(),
                };
                button(
                    row![]
                        .push(icon.size(FONT_ICON).line_height(1.0))
                        .push(text(label).size(FONT_BODY).line_height(1.0))
                        .spacing(SPACE_LG)
                        .width(Length::Fill)
                        .align_y(Alignment::Center),
                )
                .on_press(Message::Nav(NavMsg::SetSettingsCategory(target)))
                .padding(PADDING_FILTER)
                .height(Length::Fixed(FILTER_ITEM_H))
                .width(Length::Fill)
                .style(theme::style::button::filter(is_active))
                .into()
            };

            column![]
                .spacing(SPACE_MD)
                .push(make_cat(fluent.get(Tr::General), SettingsCategory::General))
                .push(make_cat(
                    fluent.get(Tr::DownloadCategory),
                    SettingsCategory::Download,
                ))
                .push(make_cat(
                    fluent.get(Tr::BitTorrent),
                    SettingsCategory::BitTorrent,
                ))
                .push(make_cat(fluent.get(Tr::Ed2k), SettingsCategory::Ed2k))
                .push(make_cat(fluent.get(Tr::Network), SettingsCategory::Network))
                .push(make_cat(
                    fluent.get(Tr::Advanced),
                    SettingsCategory::Advanced,
                ))
                .into()
        }
    };

    let pill_el = container(iced::widget::Space::new())
        .width(Length::Fill)
        .height(Length::Fixed(FILTER_ITEM_H))
        .style(theme::style::active_filter);

    let pill_layer =
        animation(pill, translate(pill_el, 0.0, *pill.value())).on_update(Message::PillAnim);

    let items_layer = Stack::new()
        .push(items)
        .push_under(pill_layer)
        .width(Length::Fill);

    container(
        column![]
            .spacing(SPACE_4XL)
            .push(title)
            .push(items_layer)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(PADDING_CATEGORY_BAR)
    .into()
}

/// Compose [`background`] and [`content`] into a single element using a
/// `Stack`. Equivalent to the historical all-in-one sidebar; callers that
/// want the background excluded from a scale animation should use
/// [`background`] + [`content`] directly instead.
pub fn view<'a>(
    fluent: &'a Fluent,
    theme: &iced::Theme,
    page: Page,
    task_filter: TaskFilter,
    settings_cat: SettingsCategory,
    counts: &Counts,
    pill: &'a Animated<f32>,
) -> Element<'a, Message> {
    let bg = background(theme);
    let content = content(fluent, theme, page, task_filter, settings_cat, counts, pill);
    Stack::new()
        .push(content)
        .push_under(bg)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{SettingsCategory, TaskFilter};

    #[test]
    fn task_filter_index_covers_all_variants() {
        let all = vec![
            task_filter_index(TaskFilter::All),
            task_filter_index(TaskFilter::Downloading),
            task_filter_index(TaskFilter::Completed),
        ];
        let mut deduped = all.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(all.len(), 3);
        assert_eq!(deduped.len(), 3, "duplicate index for some variant");
    }

    #[test]
    fn task_filter_index_bijective() {
        let pairs = [
            (TaskFilter::All, 0usize),
            (TaskFilter::Downloading, 1),
            (TaskFilter::Completed, 2),
        ];
        for (variant, expected) in pairs {
            assert_eq!(task_filter_index(variant), expected);
        }
    }

    #[test]
    fn settings_cat_index_covers_all_variants() {
        let all = vec![
            settings_cat_index(SettingsCategory::General),
            settings_cat_index(SettingsCategory::Download),
            settings_cat_index(SettingsCategory::BitTorrent),
            settings_cat_index(SettingsCategory::Ed2k),
            settings_cat_index(SettingsCategory::Network),
            settings_cat_index(SettingsCategory::Advanced),
        ];
        let mut deduped = all.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(all.len(), 6);
        assert_eq!(deduped.len(), 6, "duplicate index for some category");
    }

    #[test]
    fn settings_cat_index_bijective() {
        let pairs = [
            (SettingsCategory::General, 0usize),
            (SettingsCategory::Download, 1),
            (SettingsCategory::BitTorrent, 2),
            (SettingsCategory::Ed2k, 3),
            (SettingsCategory::Network, 4),
            (SettingsCategory::Advanced, 5),
        ];
        for (variant, expected) in pairs {
            assert_eq!(settings_cat_index(variant), expected);
        }
    }
}
