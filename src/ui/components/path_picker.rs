use std::path::PathBuf;

use iced::widget::{
    button, column, container, mouse_area, row, scrollable, text, text_input, Space, Text,
};
use iced::{Alignment, Element, Length};

use super::drop_down;

use super::tooltip;
use super::CONTROL_HEIGHT;
use crate::i18n::{Fluent, Tr};
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerMode {
    Folder,
    File,
    ReadOnly,
}

#[derive(Debug, Clone)]
pub enum PathPickerEvent {
    ToggleHistory,
    DismissHistory,
    SelectHistory(PathBuf),
    Browse,
    Copy(String),
    Changed,
    Open,
    Entered,
    Exited,
}

#[derive(Debug, Clone)]
pub enum PathPickerAction {
    Copy(String),
    Browse,
    Select(PathBuf),
    Open(PathBuf),
}

#[derive(Debug, Clone)]
pub struct PathPicker {
    value: String,
    mode: PickerMode,
    show_history: bool,
    history_open: bool,
    focused: bool,
    hovered: bool,
}

impl PathPicker {
    pub fn folder(value: impl Into<String>, show_history: bool) -> Self {
        Self {
            value: value.into(),
            mode: PickerMode::Folder,
            show_history,
            history_open: false,
            focused: false,
            hovered: false,
        }
    }

    pub fn file(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            mode: PickerMode::File,
            show_history: false,
            history_open: false,
            focused: false,
            hovered: false,
        }
    }

    pub fn read_only(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            mode: PickerMode::ReadOnly,
            show_history: false,
            history_open: false,
            focused: false,
            hovered: false,
        }
    }

    pub fn set_value(&mut self, v: impl Into<String>) {
        self.value = v.into();
    }

    pub fn set_hovered(&mut self, hovered: bool) {
        self.hovered = hovered;
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn close_history(&mut self) {
        self.history_open = false;
    }

    pub fn update(&mut self, event: PathPickerEvent) -> Option<PathPickerAction> {
        match event {
            PathPickerEvent::ToggleHistory if self.mode != PickerMode::ReadOnly => {
                self.focused = true;
                self.history_open = !self.history_open;
                None
            }
            PathPickerEvent::ToggleHistory => None,
            PathPickerEvent::DismissHistory => {
                self.history_open = false;
                None
            }
            PathPickerEvent::SelectHistory(p) => {
                self.focused = true;
                self.history_open = false;
                Some(PathPickerAction::Select(p))
            }
            PathPickerEvent::Browse => {
                self.focused = true;
                Some(PathPickerAction::Browse)
            }
            PathPickerEvent::Copy(s) => {
                self.focused = true;
                if s.is_empty() {
                    None
                } else {
                    Some(PathPickerAction::Copy(s))
                }
            }
            PathPickerEvent::Changed => None,
            PathPickerEvent::Open => {
                Some(PathPickerAction::Open(PathBuf::from(self.value.clone())))
            }
            PathPickerEvent::Entered => {
                self.hovered = true;
                None
            }
            PathPickerEvent::Exited => {
                self.hovered = false;
                self.focused = false;
                None
            }
        }
    }

    pub fn view<'a, M>(
        &self,
        fluent: &'a Fluent,
        theme: &'a iced::Theme,
        history: &'a [String],
        map: impl Fn(PathPickerEvent) -> M + 'a,
    ) -> Element<'a, M>
    where
        M: Clone + 'a,
    {
        let text_secondary = theme::text_secondary(theme);

        let copy_msg = map(PathPickerEvent::Copy(self.value.clone()));
        let open_msg = map(PathPickerEvent::Open);
        let browse_msg = map(PathPickerEvent::Browse);
        let toggle_msg = map(PathPickerEvent::ToggleHistory);
        let dismiss_msg = map(PathPickerEvent::DismissHistory);
        let enter_msg = map(PathPickerEvent::Entered);
        let exit_msg = map(PathPickerEvent::Exited);
        let select_msgs: Vec<M> = history
            .iter()
            .map(|p| map(PathPickerEvent::SelectHistory(PathBuf::from(p.clone()))))
            .collect();

        let input = theme::grouped_input_layout(
            text_input("", &self.value)
                .on_input(move |_s| map(PathPickerEvent::Changed))
                .style(if self.mode == PickerMode::ReadOnly {
                    theme::style::input::grouped_readonly
                } else {
                    theme::style::input::grouped
                })
                .width(Length::Fill),
        );
        let mut row = row![]
            .spacing(SPACE_NONE)
            .align_y(Alignment::Center)
            .height(Length::Fill);
        row = row.push(input);
        row = row.push(Self::separator());

        let copy_btn: Element<'a, M> = {
            let mut btn = button(Self::icon_content(
                icon::copy().size(FONT_ICON).color(text_secondary),
            ))
            .style(theme::style::button::grouped_icon(false, false))
            .height(Length::Fill);
            if !self.value.is_empty() {
                btn = btn.on_press(copy_msg);
            }
            tooltip::standard(
                btn,
                text(fluent.get(Tr::Copy)),
                iced::widget::tooltip::Position::Bottom,
            )
        };
        row = row.push(copy_btn);

        let reveal_btn: Element<'a, M> = {
            let mut btn = button(Self::icon_content(
                icon::folder_search().size(FONT_ICON).color(text_secondary),
            ))
            .style(theme::style::button::grouped_icon(false, false))
            .height(Length::Fill);
            if !self.value.is_empty() {
                btn = btn.on_press(open_msg);
            }
            tooltip::standard(
                btn,
                text(fluent.get(Tr::ShowInFolder)),
                iced::widget::tooltip::Position::Bottom,
            )
        };

        if self.mode != PickerMode::ReadOnly {
            row = row.push(Self::separator());
            row = row.push(reveal_btn);
            row = row.push(Self::separator());

            let browse_btn: Element<'a, M> = tooltip::standard(
                button(Self::icon_content(
                    icon::folder_open().size(FONT_ICON).color(text_secondary),
                ))
                .on_press(browse_msg)
                .style(theme::style::button::grouped_icon(false, false))
                .height(Length::Fill),
                text(fluent.get(Tr::Browse)),
                iced::widget::tooltip::Position::Bottom,
            );
            row = row.push(browse_btn);

            if self.show_history {
                row = row.push(Self::separator());
                let history_btn: Element<'a, M> = {
                    let btn = button(Self::icon_content(
                        icon::folder_clock().size(FONT_ICON).color(text_secondary),
                    ))
                    .style(theme::style::button::grouped_icon(true, false))
                    .height(Length::Fill);
                    if history.is_empty() {
                        btn.into()
                    } else {
                        btn.on_press(toggle_msg).into()
                    }
                };
                let history_btn = tooltip::standard(
                    history_btn,
                    text(fluent.get(Tr::DownloadHistory)),
                    iced::widget::tooltip::Position::Bottom,
                );
                row = row.push(history_btn);
            }
        } else {
            row = row.push(Self::separator());
            row = row.push(reveal_btn);
        }

        let group = container(row)
            .width(Length::Fill)
            .height(Length::Fixed(CONTROL_HEIGHT))
            .padding(PADDING_GROUPED)
            .style(theme::style::grouped_frame_state(
                self.focused,
                self.hovered,
            ));

        let inner: Element<'a, M> =
            if self.mode != PickerMode::ReadOnly && self.show_history && !history.is_empty() {
                let overlay_items: Vec<Element<'a, M>> = history
                    .iter()
                    .zip(&select_msgs)
                    .map(|(p, msg)| {
                        button(text(p.as_str()).size(FONT_SMALL))
                            .on_press(msg.clone())
                            .width(Length::Fill)
                            .padding(PADDING_BUTTON_XS)
                            .style(theme::style::button::picker_item())
                            .into()
                    })
                    .collect();

                let overlay = container(
                    scrollable(column(overlay_items).spacing(SPACE_XS).width(Length::Fill))
                        .direction(scrollable::Direction::Vertical(
                            scrollable::Scrollbar::hidden(),
                        )),
                )
                .padding(PADDING_DROPDOWN)
                .style(theme::style::card);

                drop_down::DropDown::new(group, overlay, self.history_open)
                    .alignment(drop_down::Alignment::Bottom)
                    .offset(drop_down::Offset::from(0.0))
                    .on_dismiss(dismiss_msg)
                    .into()
            } else {
                group.into()
            };

        mouse_area(inner)
            .on_enter(enter_msg)
            .on_exit(exit_msg)
            .into()
    }

    fn icon_content<'a, M: 'a>(icon: Text<'a>) -> Element<'a, M> {
        container(icon.line_height(1.0))
            .center_y(Length::Fill)
            .into()
    }

    fn separator<'a, M: 'a>() -> Element<'a, M> {
        container(Space::new())
            .width(Length::Fixed(1.0))
            .height(Length::Fill)
            .style(theme::style::separator)
            .into()
    }
}
