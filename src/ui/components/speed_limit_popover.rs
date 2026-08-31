use iced::widget::{column, pick_list, row, text};
use iced::{Alignment, Element, Length};

use crate::app::Remotrix;
use crate::i18n::{Fluent, Tr};
use crate::message::{DialogMsg, Message, SettingKey, SpeedUnit};
use crate::ui::components::number_stepper::number_stepper;
use crate::ui::dims::*;
use crate::ui::theme;

#[derive(Clone, Copy)]
struct UnitOption {
    value: SpeedUnit,
    label: &'static str,
}

impl std::fmt::Display for UnitOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label)
    }
}

impl PartialEq for UnitOption {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

fn estimate_label_width(s: &str) -> f32 {
    let mut w = 0.0;
    for c in s.chars() {
        w += if (c as u32) < 0x80 { 6.5 } else { 13.0 };
    }
    w
}

fn compact_row<'a>(
    label: String,
    value_kb: u64,
    unit: SpeedUnit,
    label_w: f32,
    on_value: impl Fn(u64) -> Message + 'a,
    on_unit: impl Fn(SpeedUnit) -> Message + 'a,
) -> Element<'a, Message> {
    let unit_opts = [
        UnitOption {
            value: SpeedUnit::Kbps,
            label: "KB/s",
        },
        UnitOption {
            value: SpeedUnit::Mbps,
            label: "MB/s",
        },
    ];
    let sel = unit_opts.iter().find(|o| o.value == unit).copied();

    let (display_val, step) = match unit {
        SpeedUnit::Kbps => (value_kb, 100),
        SpeedUnit::Mbps => {
            if value_kb == 0 {
                (0, 1)
            } else {
                (value_kb / 1024, 1)
            }
        }
    };

    row![]
        .spacing(SPACE_MD)
        .align_y(Alignment::Center)
        .push(text(label).size(FONT_MEDIUM).width(Length::Fixed(label_w)))
        .push(number_stepper(
            display_val,
            0..=u64::MAX,
            step,
            on_value,
            Length::Fixed(140.0),
        ))
        .push(
            pick_list(unit_opts, sel, move |o| on_unit(o.value))
                .text_size(FONT_MEDIUM)
                .padding(theme::INPUT_PADDING)
                .width(Length::Fixed(72.0))
                .style(theme::style::pick_list::standard)
                .menu_style(theme::style::pick_list::menu),
        )
        .into()
}

pub fn view<'a>(fluent: &'a Fluent, state: &'a Remotrix) -> Element<'a, Message> {
    let unit_dl = state
        .settings_ui
        .speed_units
        .get(&SettingKey::DownloadLimit)
        .copied()
        .unwrap_or(SpeedUnit::Kbps);
    let unit_ul = state
        .settings_ui
        .speed_units
        .get(&SettingKey::UploadLimit)
        .copied()
        .unwrap_or(SpeedUnit::Kbps);

    let label_dl = fluent.get(Tr::DownloadLimit);
    let label_ul = fluent.get(Tr::UploadLimit);
    let label_w = estimate_label_width(&label_dl).max(estimate_label_width(&label_ul));

    let dl_row = compact_row(
        label_dl,
        state.settings.download_limit_kb,
        unit_dl,
        label_w,
        move |v| {
            Message::Dialog(DialogMsg::SpeedLimitChanged(
                SettingKey::DownloadLimit,
                if unit_dl == SpeedUnit::Mbps {
                    v.saturating_mul(1024)
                } else {
                    v
                },
            ))
        },
        |u| {
            Message::Dialog(DialogMsg::SpeedLimitUnitChanged(
                SettingKey::DownloadLimit,
                u,
            ))
        },
    );
    let ul_row = compact_row(
        label_ul,
        state.settings.upload_limit_kb,
        unit_ul,
        label_w,
        move |v| {
            Message::Dialog(DialogMsg::SpeedLimitChanged(
                SettingKey::UploadLimit,
                if unit_ul == SpeedUnit::Mbps {
                    v.saturating_mul(1024)
                } else {
                    v
                },
            ))
        },
        |u| Message::Dialog(DialogMsg::SpeedLimitUnitChanged(SettingKey::UploadLimit, u)),
    );

    column![].spacing(SPACE_MD).push(dl_row).push(ul_row).into()
}
