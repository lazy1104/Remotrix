use iced::widget::{button, column, container, radio, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length};

use crate::config::Settings;
use crate::i18n::{Fluent, Locale, Tr};
use crate::message::{Message, SettingKey};
use crate::ui::theme;

#[allow(clippy::too_many_arguments)]
pub fn view<'a>(
    fluent: &'a Fluent,
    theme: &iced::Theme,
    settings: &'a Settings,
    aria2_version: Option<&'a str>,
    aria2_check_msg: Option<&'a str>,
    auto_check_disabled: bool,
    aria2_status: Option<(&'a str, &'a str)>,
    aria2_fetch_error: Option<&'a str>,
    update_pending: Option<&'a str>,
) -> Element<'a, Message> {
    let text_secondary = theme::text_secondary(theme);

    let dir_str = settings.download_dir.to_string_lossy().to_string();
    let dl_limit_str = settings.download_limit_kb.to_string();
    let ul_limit_str = settings.upload_limit_kb.to_string();
    let max_concurrent_str = settings.max_concurrent.to_string();

    let general = column![]
        .spacing(12)
        .push(group_title(fluent, Tr::General))
        .push(
            row![]
                .push(
                    text(fluent.get(Tr::DownloadFolder))
                        .size(13)
                        .width(Length::Fixed(180.0)),
                )
                .push(
                    text(dir_str)
                        .size(13)
                        .style(theme::style::text::secondary)
                        .width(Length::Fill),
                )
                .push(
                    button(text(fluent.get(Tr::Browse)).size(12))
                        .on_press(Message::SettingChanged(
                            SettingKey::DownloadDir,
                            String::new(),
                        ))
                        .padding([6, 12])
                        .style(button::secondary),
                )
                .align_y(Alignment::Center),
        )
        .push(
            row![]
                .push(
                    text(fluent.get(Tr::MaxConcurrent))
                        .size(13)
                        .width(Length::Fixed(180.0)),
                )
                .push(
                    text_input("5", max_concurrent_str.as_str())
                        .on_input(|s| Message::SettingChanged(SettingKey::MaxConcurrent, s))
                        .width(Length::Fixed(120.0))
                        .padding(8)
                        .size(13),
                )
                .align_y(Alignment::Center),
        );

    let speed = column![]
        .spacing(12)
        .push(group_title(fluent, Tr::SpeedLimits))
        .push(
            row![]
                .push(
                    text(fluent.get(Tr::DownloadLimit))
                        .size(13)
                        .width(Length::Fixed(240.0)),
                )
                .push(
                    text_input("0", dl_limit_str.as_str())
                        .on_input(|s| Message::SettingChanged(SettingKey::DownloadLimit, s))
                        .width(Length::Fixed(120.0))
                        .padding(8)
                        .size(13),
                )
                .align_y(Alignment::Center),
        )
        .push(
            row![]
                .push(
                    text(fluent.get(Tr::UploadLimit))
                        .size(13)
                        .width(Length::Fixed(240.0)),
                )
                .push(
                    text_input("0", ul_limit_str.as_str())
                        .on_input(|s| Message::SettingChanged(SettingKey::UploadLimit, s))
                        .width(Length::Fixed(120.0))
                        .padding(8)
                        .size(13),
                )
                .align_y(Alignment::Center),
        );

    let theme_group = group_title(fluent, Tr::Theme);
    let theme_options = row![]
        .push(radio(
            fluent.get(Tr::ThemeDark).to_string(),
            crate::ui::theme::ThemeMode::Dark,
            Some(settings.theme_mode),
            Message::ThemeModeChanged,
        ))
        .push(iced::widget::Space::new().width(Length::Fixed(24.0)))
        .push(radio(
            fluent.get(Tr::ThemeLight).to_string(),
            crate::ui::theme::ThemeMode::Light,
            Some(settings.theme_mode),
            Message::ThemeModeChanged,
        ))
        .push(iced::widget::Space::new().width(Length::Fixed(24.0)))
        .push(radio(
            fluent.get(Tr::ThemeSystem).to_string(),
            crate::ui::theme::ThemeMode::System,
            Some(settings.theme_mode),
            Message::ThemeModeChanged,
        ))
        .align_y(Alignment::Center);

    let locale_group = group_title(fluent, Tr::Locale);
    let locale_options = row![]
        .push(radio(
            fluent.get(Tr::LocaleZh).to_string(),
            Locale::ZhCN,
            Some(settings.locale),
            Message::LocaleChanged,
        ))
        .push(iced::widget::Space::new().width(Length::Fixed(24.0)))
        .push(radio(
            fluent.get(Tr::LocaleEn).to_string(),
            Locale::EnUS,
            Some(settings.locale),
            Message::LocaleChanged,
        ))
        .align_y(Alignment::Center);

    let appearance = column![]
        .spacing(12)
        .push(theme_group)
        .push(theme_options)
        .push(locale_group)
        .push(locale_options);

    let mut engine_rows: Vec<Element<Message>> = Vec::new();

    let version_text = match aria2_version {
        Some(v) => format!("aria2-next v{v}"),
        None => "aria2-next (--)".to_string(),
    };
    engine_rows.push(
        row![]
            .push(
                text(fluent.get(Tr::Aria2Version))
                    .size(13)
                    .width(Length::Fixed(180.0)),
            )
            .push(
                text(version_text)
                    .size(13)
                    .style(theme::style::text::secondary),
            )
            .align_y(Alignment::Center)
            .into(),
    );

    if let Some((stage, message)) = aria2_status {
        let status_color = if stage == "update-downloading" || stage == "update-verifying" {
            theme::ACCENT
        } else if stage == "ready" {
            theme::PROGRESS
        } else {
            text_secondary
        };
        engine_rows.push(text(message).size(12).color(status_color).into());
    }

    if let Some(err) = aria2_fetch_error {
        engine_rows.push(text(err).size(12).color(theme::ERROR).into());
    }

    let mut btn_row = row![].spacing(12);

    if let Some(pending) = update_pending {
        btn_row = btn_row.push(
            button(text(fluent.get(Tr::RestartToUpdate)).size(12))
                .on_press(Message::RestartEngine)
                .padding([6, 12])
                .style(button::primary),
        );
        btn_row = btn_row.push(
            text(format!(
                "v{pending} - {}",
                fluent.get(Tr::PendingUpdateHint)
            ))
            .size(12)
            .style(theme::style::text::secondary),
        );
    } else if aria2_fetch_error.is_some() {
        btn_row = btn_row.push(
            button(text(fluent.get(Tr::Retry)).size(12))
                .on_press(Message::RetryAria2Fetch)
                .padding([6, 12])
                .style(button::secondary),
        );
    } else {
        btn_row = btn_row.push(
            button(text(fluent.get(Tr::CheckUpdate)).size(12))
                .on_press(Message::CheckAria2Update)
                .padding([6, 12])
                .style(button::secondary),
        );
    }

    if auto_check_disabled {
        btn_row = btn_row.push(
            button(text(fluent.get(Tr::RestoreAutoCheck)).size(12))
                .on_press(Message::RestoreAutoCheck)
                .padding([6, 12])
                .style(button::secondary),
        );
    }

    if let Some(msg) = aria2_check_msg {
        btn_row = btn_row.push(text(msg).size(12).style(theme::style::text::secondary));
    }

    engine_rows.push(btn_row.into());

    let mut engine_col = column![].spacing(8);
    for elem in engine_rows {
        engine_col = engine_col.push(elem);
    }

    let engine = column![]
        .spacing(12)
        .push(group_title(fluent, Tr::Engine))
        .push(engine_col);

    let apply = button(text(fluent.get(Tr::Apply)).size(14))
        .on_press(Message::ApplySettings)
        .padding([10, 24])
        .style(button::primary);

    let content = column![]
        .spacing(28)
        .push(general)
        .push(speed)
        .push(appearance)
        .push(engine)
        .push(
            row![]
                .push(iced::widget::Space::new().width(Length::Fill))
                .push(apply)
                .width(Length::Fill),
        );

    container(
        column![]
            .push(text(fluent.get(Tr::SettingsTitle)).size(22))
            .push(iced::widget::Space::new().height(Length::Fixed(20.0)))
            .push(scrollable(content).height(Length::Fill)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding([24, 28])
    .into()
}

fn group_title<'a>(fluent: &'a Fluent, key: Tr) -> Element<'a, Message> {
    text(fluent.get(key)).size(16).color(theme::ACCENT).into()
}
