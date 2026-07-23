use iced::widget::{button, column, container, radio, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length};

use crate::config::Settings;
use crate::i18n::{Fluent, Locale, Tr};
use crate::message::{Message, SettingKey};
use crate::ui::theme;

pub fn view<'a>(fluent: &'a Fluent, dark: bool, settings: &'a Settings) -> Element<'a, Message> {
    let text_primary = if dark {
        theme::TEXT_PRIMARY
    } else {
        theme::TEXT_PRIMARY_LIGHT
    };
    let text_secondary = if dark {
        theme::TEXT_SECONDARY
    } else {
        theme::TEXT_SECONDARY_LIGHT
    };

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
                        .width(Length::Fixed(180.0))
                        .color(text_primary),
                )
                .push(
                    text(dir_str)
                        .size(13)
                        .color(text_secondary)
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
                        .width(Length::Fixed(180.0))
                        .color(text_primary),
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
                        .width(Length::Fixed(240.0))
                        .color(text_primary),
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
                        .width(Length::Fixed(240.0))
                        .color(text_primary),
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

    let apply = button(text(fluent.get(Tr::Apply)).size(14))
        .on_press(Message::ApplySettings)
        .padding([10, 24])
        .style(button::primary);

    let content = column![]
        .spacing(28)
        .push(general)
        .push(speed)
        .push(appearance)
        .push(
            row![]
                .push(iced::widget::Space::new().width(Length::Fill))
                .push(apply)
                .width(Length::Fill),
        );

    container(
        column![]
            .push(
                text(fluent.get(Tr::SettingsTitle))
                    .size(22)
                    .color(text_primary),
            )
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
