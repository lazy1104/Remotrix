use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{Message, Page, SettingsCategory, TaskFilter};
use crate::ui::theme;

pub struct Counts {
    pub all: usize,
    pub downloading: usize,
    pub completed: usize,
}

pub fn view<'a>(
    fluent: &'a Fluent,
    _theme: &iced::Theme,
    page: Page,
    task_filter: TaskFilter,
    _settings_cat: SettingsCategory,
    counts: &Counts,
) -> Element<'a, Message> {
    let title_str = match page {
        Page::Tasks => fluent.get(Tr::TasksList),
        Page::Settings => fluent.get(Tr::Preferences),
    };

    let title = text(title_str).size(16).font(iced::Font {
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
                    let btn: iced::widget::Button<'_, Message> = button(
                        row![]
                            .push(text(label_text).size(14))
                            .push(iced::widget::Space::new().width(Length::Fill))
                            .align_y(Alignment::Center)
                            .width(Length::Fill),
                    )
                    .on_press(Message::SetTaskFilter(target))
                    .padding([10, 14])
                    .width(Length::Fill)
                    .style(button::text);

                    if is_active {
                        container(btn).style(theme::style::active_filter).into()
                    } else {
                        container(btn).into()
                    }
                };

            column![]
                .spacing(6)
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
            let is_active = true;
            let btn: iced::widget::Button<'_, Message> = button(
                row![]
                    .push(text(fluent.get(Tr::General)).size(14))
                    .width(Length::Fill)
                    .align_y(Alignment::Center),
            )
            .on_press(Message::SetSettingsCategory(SettingsCategory::General))
            .padding([10, 14])
            .width(Length::Fill)
            .style(button::text);

            let item: Element<'a, Message> = if is_active {
                container(btn).style(theme::style::active_filter).into()
            } else {
                container(btn).into()
            };

            column![].spacing(6).push(item).into()
        }
    };

    container(
        column![]
            .spacing(16)
            .push(title)
            .push(items)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding([20, 14])
    .style(theme::style::category_background)
    .into()
}
