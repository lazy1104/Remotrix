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
/// All four fields are populated by the app each time the task list
/// changes; missing entries are simply rendered without a count.
pub struct Counts {
    pub all: usize,
    pub downloading: usize,
    pub completed: usize,
    pub failed: usize,
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
                    let icon = match target {
                        TaskFilter::All => icon::layers(),
                        TaskFilter::Downloading => icon::download_arrow(),
                        TaskFilter::Completed => icon::circle_check(),
                        TaskFilter::Failed => icon::circle_alert(),
                    };
                    let mut inner = row![]
                        .push(icon.size(FONT_ICON).line_height(1.0))
                        .push(text(label).size(FONT_BODY).line_height(1.0))
                        .push(iced::widget::Space::new().width(Length::Fill))
                        .spacing(SPACE_LG)
                        .align_y(Alignment::Center)
                        .width(Length::Fill);
                    if count > 0 {
                        let display = if count > 99 {
                            "99+".to_string()
                        } else {
                            count.to_string()
                        };
                        let badge = container(
                            text(display)
                                .size(FONT_TINY)
                                .line_height(1.0)
                                .align_x(iced::alignment::Horizontal::Center)
                                .width(Length::Fill),
                        )
                        .width(Length::Fixed(COUNT_BADGE_W))
                        .height(Length::Fixed(24.0))
                        .padding(COUNT_BADGE_PAD)
                        .align_x(Alignment::Center)
                        .align_y(Alignment::Center)
                        .style(theme::style::count_badge(is_active));
                        inner = inner.push(badge);
                    }
                    button(inner)
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
                .push(make_filter(
                    fluent.get(Tr::Failed),
                    counts.failed,
                    TaskFilter::Failed,
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
