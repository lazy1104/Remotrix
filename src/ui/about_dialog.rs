//! "About" dialog: app logo, version, runtime engine version, an aria2
//! update progress bar, and external links to the upstream projects.

use iced::alignment::Alignment;
use iced::widget::{button, column, container, row, text};
use iced::{Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{DialogMsg, Message};
use crate::ui::components::copyable_text::copyable_text;
use crate::ui::components::dialog::Dialog;
use crate::ui::components::expand::expand_pinned;
use crate::ui::components::logo;
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

const ICED_REPO_URL: &str = "https://github.com/iced-rs/iced";
const ARIA2_NEXT_REPO_URL: &str = "https://github.com/AnInsomniacy/aria2-next";
const REMOTRIX_REPO_URL: &str = "https://github.com/lazy1104/Remotrix";

fn repo_link<'a>(url: String) -> Element<'a, Message, iced::Theme, iced::Renderer> {
    button(icon::link().size(FONT_MEDIUM))
        .on_press(Message::OpenLink(url))
        .padding(PADDING_BUTTON_SM)
        .style(theme::style::button::copyable())
        .into()
}

/// Build the about dialog element. `progress` is the aria2 download
/// progress (0..=1); pass `0.0` when no update is in flight.
pub fn view<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    aria2_version: Option<&'a str>,
    progress: f32,
) -> Element<'a, Message> {
    let gui_text = format!("Remotrix {}", env!("CARGO_PKG_VERSION"));
    let engine_text = match aria2_version {
        Some(v) => format!("aria2-next v{v}"),
        None => "aria2-next (--)".to_string(),
    };
    let iced_text = format!("iced {}", "0.14");

    let iced_row = row![
        copyable_text(iced_text.clone(), Message::CopyText(iced_text)).width(Length::Fill),
        repo_link(ICED_REPO_URL.to_string()),
    ]
    .spacing(SPACE_MD)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let engine_row = row![
        copyable_text(engine_text.clone(), Message::CopyText(engine_text)).width(Length::Fill),
        repo_link(ARIA2_NEXT_REPO_URL.to_string()),
    ]
    .spacing(SPACE_MD)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let remotrix_row = row![
        copyable_text(gui_text.clone(), Message::CopyText(gui_text)).width(Length::Fill),
        repo_link(REMOTRIX_REPO_URL.to_string()),
    ]
    .spacing(SPACE_MD)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let body = column![]
        .spacing(SPACE_4XL)
        .align_x(Alignment::Center)
        .width(Length::Fill)
        .push(
            container(logo::view_brand(ABOUT_LOGO_SIZE, ABOUT_LOGO_SIZE))
                .center_x(Length::Fill)
                .width(Length::Fill),
        )
        .push(
            row![
                text("Re").color(theme::primary(theme)).size(FONT_TITLE),
                text("Motrix").size(FONT_TITLE),
            ]
            .spacing(0)
            .align_y(Alignment::Center),
        )
        .push(
            text(fluent.get(Tr::AboutTagline))
                .size(FONT_SMALL)
                .style(theme::style::text::secondary),
        )
        .push(remotrix_row)
        .push(
            text(fluent.get(Tr::CoreDependencies))
                .size(FONT_SMALL)
                .style(theme::style::text::secondary),
        )
        .push(iced_row)
        .push(engine_row)
        .push(
            text(fluent.get(Tr::LicenseNotice))
                .size(FONT_SMALL)
                .style(theme::style::text::secondary),
        );

    expand_pinned(
        Dialog::new()
            .width(380.0)
            .with_close(Message::Dialog(DialogMsg::CloseAbout))
            .body(body)
            .build(),
        progress,
    )
}
