use std::path::PathBuf;

use iced::widget::{
    button, column, container, mouse_area, row, text, text_input, Space, Text,
};
use iced::{Alignment, Element, Length};

use iced_aw::widget::drop_down;

use super::tooltip;
use super::CONTROL_HEIGHT;
use crate::i18n::{Fluent, Tr};
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
    Entered,
    Exited,
}

#[derive(Debug, Clone)]
pub enum PathPickerAction {
    Copy(String),
    Browse,
    Select(PathBuf),
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

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn close_history(&mut self) {
        self.history_open = false;
    }

    pub fn is_history_open(&self) -> bool {
        self.history_open
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
        let mut row = row![]
            .spacing(0)
            .align_y(Alignment::Center)
            .height(Length::Fill);

        let input = text_input("", &self.value)
            .style(theme::style::input::grouped)
            .width(Length::Fill)
            .padding([0, 10])
            .size(13);
        row = row.push(input);
        row = row.push(Self::separator());

        let copy_btn: Element<'a, M> = {
            let mut btn = button(Self::icon_content(
                icon::copy().size(15).color(text_secondary),
            ))
            .style(theme::style::button::grouped_icon(false))
            .height(Length::Fill);
            if !self.value.is_empty() {
                btn = btn.on_press(map(PathPickerEvent::Copy(self.value.clone())));
            }
            tooltip::standard(btn, text(fluent.get(Tr::Copy)), iced::widget::tooltip::Position::Bottom)
        };
        row = row.push(copy_btn);

        if self.mode != PickerMode::ReadOnly {
            row = row.push(Self::separator());

            let browse_btn: Element<'a, M> = tooltip::standard(
                button(Self::icon_content(
                    icon::folder_open().size(15).color(text_secondary),
                ))
                .on_press(map(PathPickerEvent::Browse))
                .style(theme::style::button::grouped_icon(false))
                .height(Length::Fill),
                text(fluent.get(Tr::Browse)),
                iced::widget::tooltip::Position::Bottom,
            );
            row = row.push(browse_btn);

            if self.show_history {
                row = row.push(Self::separator());
                if history.is_empty() {
                    let disabled_btn = button(Self::icon_content(
                        icon::folder_clock().size(15).color(text_secondary),
                    ))
                    .style(theme::style::button::grouped_icon(true))
                    .height(Length::Fill);
                    row = row.push(disabled_btn);
                } else {
                    let trailing_btn = button(Self::icon_content(
                        icon::folder_clock().size(15).color(text_secondary),
                    ))
                    .on_press(map(PathPickerEvent::ToggleHistory))
                    .style(theme::style::button::grouped_icon(true))
                    .height(Length::Fill);
                    row = row.push(trailing_btn);
                }
            }
        }

        let group = container(row)
            .width(Length::Fill)
            .height(Length::Fixed(CONTROL_HEIGHT))
            .padding(1.0)
            .style(theme::style::grouped_frame_state(
                self.focused,
                self.hovered,
            ));

        let inner: Element<'a, M> =
            if self.mode != PickerMode::ReadOnly && self.show_history && !history.is_empty() {
                let overlay_items: Vec<Element<'a, M>> = history
                    .iter()
                    .map(|p| {
                        button(text(p.as_str()).size(12))
                            .on_press(map(PathPickerEvent::SelectHistory(PathBuf::from(
                                p.clone(),
                            ))))
                            .width(Length::Fill)
                            .padding([6, 8])
                            .style(theme::style::button::text())
                            .into()
                    })
                    .collect();

                let overlay = container(column(overlay_items).spacing(2).width(Length::Fill))
                    .padding(6)
                    .style(theme::style::card);

                drop_down::DropDown::new(group, overlay, self.history_open)
                    .on_dismiss(map(PathPickerEvent::DismissHistory))
                    .into()
            } else {
                group.into()
            };

        if self.mode != PickerMode::ReadOnly {
            mouse_area(inner)
                .on_enter(map(PathPickerEvent::Entered))
                .on_exit(map(PathPickerEvent::Exited))
                .into()
        } else {
            inner
        }
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
