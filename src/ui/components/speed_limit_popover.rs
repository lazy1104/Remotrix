use iced::widget::{column, container, pick_list, row, text};
use iced::{Alignment, Element, Length};

use crate::app::Remotrix;
use crate::i18n::{Fluent, Tr};
use crate::message::{DialogMsg, Message, SettingKey, SpeedUnit};
use crate::ui::components::number_stepper::number_stepper;
use crate::ui::dims::*;
use crate::ui::theme;
use crate::ui::components::CONTROL_HEIGHT;

const STEP_W: f32 = 140.0;
const UNIT_W: f32 = 72.0;

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

    let opts = [
        UnitOption {
            value: SpeedUnit::Kbps,
            label: "KB/s",
        },
        UnitOption {
            value: SpeedUnit::Mbps,
            label: "MB/s",
        },
    ];
    let sel_dl = opts.iter().find(|o| o.value == unit_dl).copied();
    let sel_ul = opts.iter().find(|o| o.value == unit_ul).copied();

    let (val_dl, step_dl) = match unit_dl {
        SpeedUnit::Kbps => (state.settings.download_limit_kb, 100),
        SpeedUnit::Mbps => {
            if state.settings.download_limit_kb == 0 {
                (0, 1)
            } else {
                (state.settings.download_limit_kb / 1024, 1)
            }
        }
    };
    let (val_ul, step_ul) = match unit_ul {
        SpeedUnit::Kbps => (state.settings.upload_limit_kb, 100),
        SpeedUnit::Mbps => {
            if state.settings.upload_limit_kb == 0 {
                (0, 1)
            } else {
                (state.settings.upload_limit_kb / 1024, 1)
            }
        }
    };

    let on_val_dl = move |v: u64| {
        Message::Dialog(DialogMsg::SpeedLimitChanged(
            SettingKey::DownloadLimit,
            if unit_dl == SpeedUnit::Mbps {
                v.saturating_mul(1024)
            } else {
                v
            },
        ))
    };
    let on_val_ul = move |v: u64| {
        Message::Dialog(DialogMsg::SpeedLimitChanged(
            SettingKey::UploadLimit,
            if unit_ul == SpeedUnit::Mbps {
                v.saturating_mul(1024)
            } else {
                v
            },
        ))
    };
    let on_unit_dl = |u: SpeedUnit| {
        Message::Dialog(DialogMsg::SpeedLimitUnitChanged(
            SettingKey::DownloadLimit,
            u,
        ))
    };
    let on_unit_ul = |u: SpeedUnit| {
        Message::Dialog(DialogMsg::SpeedLimitUnitChanged(SettingKey::UploadLimit, u))
    };

    let labels_col = column![
        container(text(fluent.get(Tr::DownloadLimit)).size(FONT_MEDIUM))
        .height(Length::Fixed(CONTROL_HEIGHT))
        .align_y(Alignment::Center),
        container(text(fluent.get(Tr::UploadLimit)).size(FONT_MEDIUM))
        .height(Length::Fixed(CONTROL_HEIGHT))
        .align_y(Alignment::Center),
    ]
    .spacing(SPACE_MD);

    let steppers_col = column![
        number_stepper(
            val_dl,
            0..=u64::MAX,
            step_dl,
            on_val_dl,
            Length::Fixed(STEP_W),
        ),
        number_stepper(
            val_ul,
            0..=u64::MAX,
            step_ul,
            on_val_ul,
            Length::Fixed(STEP_W),
        ),
    ]
    .spacing(SPACE_MD);

    let units_col = column![
        pick_list(opts, sel_dl, move |o| on_unit_dl(o.value))
            .text_size(FONT_MEDIUM)
            .padding(theme::INPUT_PADDING)
            .width(Length::Fixed(UNIT_W))
            .style(theme::style::pick_list::standard)
            .menu_style(theme::style::pick_list::menu),
        pick_list(opts, sel_ul, move |o| on_unit_ul(o.value))
            .text_size(FONT_MEDIUM)
            .padding(theme::INPUT_PADDING)
            .width(Length::Fixed(UNIT_W))
            .style(theme::style::pick_list::standard)
            .menu_style(theme::style::pick_list::menu),
    ]
    .spacing(SPACE_MD);

    row![labels_col, steppers_col, units_col]
        .spacing(SPACE_MD)
        .align_y(Alignment::Center)
        .into()
}
