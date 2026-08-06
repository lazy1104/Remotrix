use iced::widget::{button, column, container, row, text, Stack};
use iced::{Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{Message, NavMsg, Page, SettingsCategory, TaskFilter};
use crate::ui::animation::{animation, Animated};
use crate::ui::components::translate::translate;
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

pub struct Counts {
    pub all: usize,
    pub downloading: usize,
    pub completed: usize,
}

pub fn task_filter_index(f: TaskFilter) -> usize {
    match f {
        TaskFilter::All => 0,
        TaskFilter::Downloading => 1,
        TaskFilter::Completed => 2,
    }
}

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

pub fn view<'a>(
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
                            .push(icon.size(FONT_ICON))
                            .push(text(label_text).size(FONT_BODY))
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
                        .push(icon.size(FONT_ICON))
                        .push(text(label).size(FONT_BODY))
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
    .style(theme::style::category_background)
    .into()
}
