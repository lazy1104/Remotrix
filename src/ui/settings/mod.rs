//! Settings page: category tabs, live edit of [`Settings`], with a
//! pending/dirty buffer that requires an explicit Apply.
//!
//! `app.rs` owns the persisted [`Settings`] and the working copy in
//! [`SettingsUiState`]; this module just renders the editors and posts
//! [`SettingsMsg`]s.

use std::collections::HashMap;

use iced::widget::{
    button, checkbox, column, container, mouse_area, pick_list, row, text, text_editor, text_input,
    toggler,
};
use iced::{Alignment, Element, Length};

use crate::config::Settings;
use crate::engine::EngineCmd;
use crate::i18n::{Fluent, Locale, Tr};
use crate::message::{
    AddMsg, CtxTarget, EngineMsg, Message, PathPickerId, SettingKey, SettingValue,
    SettingsCategory, SettingsMsg, SpeedUnit,
};
use crate::task::format_size;
use chrono::TimeZone;
use iced::Color;

use crate::ui::components::color_picker::CustomColorPickerUi;
use crate::ui::components::copyable_text::copyable_text;
use crate::ui::components::ctx_input;
use crate::ui::components::ctx_menu::CtxMirrors;
use crate::ui::components::font_picker::{self as font_picker, FontPickerOption, FontPickerUi};
use crate::ui::components::number_stepper::number_stepper;
use crate::ui::components::path_picker::PathPicker;
use crate::ui::components::slim_scrollable::slim_scrollable;
use crate::ui::components::swatch_icon::{swatch_check, swatch_plus, swatch_text_color};
use crate::ui::components::tag_picker::tag_picker;
use crate::ui::components::tooltip;
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

pub mod advanced;
pub mod bittorrent;
pub mod download;
pub mod ed2k;
pub mod general;
pub mod network;

pub const SETTINGS_SCROLL_ID: &str = "settings-scroll";

/// Per-screen UI state for the settings page: path pickers, transient
/// edit fields, and popover open flags. Persisted settings live in
/// [`crate::config::Settings`]; this struct is rebuilt on demand.
#[derive(Debug, Clone)]
pub struct SettingsUiState {
    pub download_picker: PathPicker,
    pub ed2k_server_list_picker: PathPicker,
    pub ed2k_node_list_picker: PathPicker,
    pub aria2_dir_picker: PathPicker,
    pub app_data_dir_picker: PathPicker,
    pub log_dir_picker: PathPicker,
    pub speed_units: HashMap<SettingKey, SpeedUnit>,
    pub schedule_days_menu_open: bool,
    pub custom_tracker_input: String,
    pub syncing_trackers: bool,
    pub syncing_bootstrap: bool,
    pub tracker_sync_toast_id: Option<u64>,
    pub ed2k_search_state: Ed2kSearchUiState,
    pub ed2k_bootstrap_status: (Option<i64>, Option<i64>),
    pub custom_color_picker: CustomColorPickerUi,
    pub font_picker: FontPickerUi,
}

#[derive(Debug, Clone)]
pub struct Ed2kSearchUiState {
    pub keyword: String,
    pub file_type: String,
    pub min_sources: u32,
    pub timeout_secs: u32,
    pub sessions: HashMap<String, Ed2kSearchSession>,
}

#[derive(Debug, Clone)]
pub struct Ed2kSearchSession {
    #[allow(dead_code)]
    pub keyword: String,
    pub results: serde_json::Value,
    #[allow(dead_code)]
    pub started_at_ms: i64,
}

impl Ed2kSearchUiState {
    fn new() -> Self {
        Self {
            keyword: String::new(),
            file_type: "any".to_string(),
            min_sources: 10,
            timeout_secs: 20,
            sessions: HashMap::new(),
        }
    }

    pub fn update(&mut self, key: SettingKey, value: SettingValue) {
        match key {
            SettingKey::Ed2kSearchKeyword => {
                if let SettingValue::Text(s) = value {
                    self.keyword = s;
                }
            }
            SettingKey::Ed2kSearchFileType => {
                if let SettingValue::Text(s) = value {
                    self.file_type = s;
                }
            }
            SettingKey::Ed2kSearchMinSources => {
                if let SettingValue::Num(n) = value {
                    self.min_sources = n.clamp(1, 1000) as u32;
                }
            }
            SettingKey::Ed2kSearchTimeout => {
                if let SettingValue::Num(n) = value {
                    self.timeout_secs = n.clamp(10, 600) as u32;
                }
            }
            _ => {}
        }
    }

    pub fn build_cmd(&self) -> Option<EngineCmd> {
        let keyword = self.keyword.trim();
        if keyword.is_empty() {
            return None;
        }
        let mut opts = serde_json::Map::new();
        let ft = self.file_type.trim();
        if !ft.is_empty() && ft != "any" {
            opts.insert(
                "fileType".to_string(),
                serde_json::Value::String(ft.to_string()),
            );
        }
        opts.insert(
            "minSourceCount".to_string(),
            serde_json::Value::String(self.min_sources.to_string()),
        );
        Some(EngineCmd::Ed2kSearchStart {
            keyword: keyword.to_string(),
            options: opts,
            timeout_secs: self.timeout_secs,
        })
    }

    pub fn cancel(&mut self) -> Vec<String> {
        self.sessions.drain().map(|(gid, _)| gid).collect()
    }
}

impl SettingsUiState {
    pub fn new(settings: &Settings) -> Self {
        let mut speed_units = HashMap::new();
        for key in &[
            SettingKey::DownloadLimit,
            SettingKey::UploadLimit,
            SettingKey::MaxDownloadLimit,
            SettingKey::MaxUploadLimit,
            SettingKey::LowestSpeedLimit,
        ] {
            speed_units.insert(*key, SpeedUnit::Kbps);
        }
        let aria2_default = settings
            .paths
            .aria2_bin_dir
            .clone()
            .unwrap_or_else(|| settings.last_resolved.aria2_bin_dir.clone());
        let appdata_default = settings
            .paths
            .app_data_dir
            .clone()
            .unwrap_or_else(|| settings.last_resolved.app_data_dir.clone());
        let logs_default = settings
            .paths
            .log_dir
            .clone()
            .unwrap_or_else(|| settings.last_resolved.log_dir.clone());
        Self {
            download_picker: PathPicker::folder(
                settings.download_dir.to_string_lossy().into_owned(),
                false,
            ),
            ed2k_server_list_picker: PathPicker::file(settings.aria2.ed2k_server_list.clone()),
            ed2k_node_list_picker: PathPicker::file(settings.aria2.ed2k_node_list.clone()),
            aria2_dir_picker: PathPicker::folder(
                aria2_default.to_string_lossy().into_owned(),
                false,
            ),
            app_data_dir_picker: PathPicker::folder(
                appdata_default.to_string_lossy().into_owned(),
                false,
            ),
            log_dir_picker: PathPicker::folder(logs_default.to_string_lossy().into_owned(), false),
            speed_units,
            schedule_days_menu_open: false,
            custom_tracker_input: String::new(),
            syncing_trackers: false,
            syncing_bootstrap: false,
            tracker_sync_toast_id: None,
            ed2k_search_state: Ed2kSearchUiState::new(),
            ed2k_bootstrap_status: crate::ed2k_bootstrap::bootstrap_status(),
            custom_color_picker: CustomColorPickerUi::default(),
            font_picker: FontPickerUi::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct Labeled<T> {
    value: T,
    label: String,
}

impl<T> std::fmt::Display for Labeled<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label)
    }
}

impl<T> PartialEq for Labeled<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

/// All inputs the settings page needs to render a frame: the current
/// settings (live and pending), the UI state, the active category, and
/// a few engine callbacks.
pub struct SettingsPageContext<'a> {
    pub fluent: &'a Fluent,
    pub theme: &'a iced::Theme,
    pub settings: &'a Settings,
    pub settings_ui: &'a SettingsUiState,
    pub category: SettingsCategory,
    pub applied_settings: &'a Settings,
    pub settings_dirty: bool,
    pub engine_restart_pending: bool,
    pub engine_restart_in_progress: bool,
    pub aria2_version: Option<&'a str>,
    pub aria2_status: Option<(&'a str, &'a str)>,
    pub aria2_fetch_error: Option<&'a str>,
    pub update_check_in_flight: bool,
    pub aria2_download_version: Option<&'a str>,
    pub aria2_download_progress: Option<(u64, u64)>,
    pub ua_editor: &'a text_editor::Content,
    pub bt_tracker_editor: &'a text_editor::Content,
    pub path_history: &'a HashMap<String, Vec<String>>,
    pub font_restart_required: bool,
    pub ctx_mirrors: &'a CtxMirrors,
    pub port_status: &'a std::collections::HashMap<
        crate::port_guard::PortKind,
        (u16, crate::port_guard::PortStatus),
    >,
}

/// Render the settings page for the active category.
pub fn view<'a>(ctx: &SettingsPageContext<'a>) -> Element<'a, Message> {
    let SettingsPageContext {
        fluent,
        theme,
        settings,
        settings_ui,
        category,
        applied_settings,
        settings_dirty,
        engine_restart_pending,
        engine_restart_in_progress,
        aria2_version,
        aria2_status,
        aria2_fetch_error,
        update_check_in_flight,
        aria2_download_version,
        aria2_download_progress,
        ua_editor,
        bt_tracker_editor,
        path_history,
        font_restart_required,
        ctx_mirrors,
        port_status,
    } = ctx;
    let accent = theme::accent(theme);
    let base_text = theme.extended_palette().background.base.text;
    let restart_icon_color = if *engine_restart_in_progress {
        Color {
            a: base_text.a * 0.5,
            ..base_text
        }
    } else {
        base_text
    };
    let dirty = *settings_dirty;
    let content = match category {
        SettingsCategory::General => general::general_view(
            fluent,
            theme,
            settings,
            applied_settings,
            settings_ui,
            *font_restart_required,
            *aria2_version,
            *update_check_in_flight,
            *aria2_download_version,
            *aria2_download_progress,
        ),
        SettingsCategory::Download => {
            download::download_view(fluent, theme, settings, settings_ui, path_history)
        }
        SettingsCategory::BitTorrent => bittorrent::bittorrent_view(
            fluent,
            settings,
            applied_settings,
            settings_ui,
            bt_tracker_editor,
            settings_ui.syncing_trackers,
            accent,
            ctx_mirrors,
        ),
        SettingsCategory::Ed2k => {
            ed2k::ed2k_view(fluent, theme, settings, settings_ui, port_status)
        }
        SettingsCategory::Network => network::network_view(fluent, settings, ua_editor, accent),
        SettingsCategory::Advanced => advanced::advanced_view(
            fluent,
            theme,
            settings,
            applied_settings,
            settings_ui,
            *engine_restart_pending,
            *aria2_status,
            *aria2_fetch_error,
            port_status,
        ),
    };
    let mut body = column![]
        .push(text(settings_title(fluent, *category)).size(FONT_PAGE_TITLE))
        .push(iced::widget::Space::new().height(Length::Fixed(SPACE_LG)))
        .push(iced::widget::rule::horizontal(1))
        .push(
            iced::widget::keyed::Column::new()
                .push(
                    *category,
                    slim_scrollable(content, iced::widget::Id::new(SETTINGS_SCROLL_ID), |_v| {
                        Message::ScrollableScrolled(iced::widget::Id::new(SETTINGS_SCROLL_ID))
                    })
                    .height(Length::Fill),
                )
                .width(Length::Fill)
                .height(Length::Fill),
        );

    let mut actions = row![].spacing(SPACE_2XL).width(Length::Fill);
    actions = actions.push(
        button(text(fluent.get(Tr::Apply)).size(FONT_BODY))
            .on_press_maybe(if dirty {
                Some(Message::Settings(SettingsMsg::ApplySettings))
            } else {
                None
            })
            .padding(PADDING_BUTTON_XL)
            .height(Length::Fixed(ACTION_BUTTON_H))
            .style(theme::style::button::primary()),
    );
    actions = actions.push(
        button(text(fluent.get(Tr::Reset)).size(FONT_BODY))
            .on_press_maybe(if dirty {
                Some(Message::Settings(SettingsMsg::ResetSettings))
            } else {
                None
            })
            .padding(PADDING_BUTTON_XL)
            .height(Length::Fixed(ACTION_BUTTON_H))
            .style(theme::style::button::secondary()),
    );
    actions = actions.push(
        button(
            row![
                crate::ui::components::spinner::Spinner::refresh(
                    restart_icon_color,
                    FONT_ICON as f32,
                )
                .animate(*engine_restart_in_progress)
                .box_factor(RESTART_ICON_BOX_FACTOR)
                .view(),
                text(fluent.get(Tr::RestartEngine)).size(FONT_BODY),
                {
                    let dot_color: Color = if *engine_restart_in_progress {
                        restart_icon_color
                    } else {
                        advanced::engine_status_color(theme, *aria2_status, *aria2_fetch_error)
                    };
                    crate::ui::components::status_dot::StatusDot::new(dot_color).view()
                },
            ]
            .spacing(SPACE_SM)
            .align_y(Alignment::Center),
        )
        .on_press_maybe(if *engine_restart_in_progress {
            None
        } else {
            Some(Message::Engine(EngineMsg::RestartEngine))
        })
        .padding(PADDING_BUTTON_XL)
        .height(Length::Fixed(ACTION_BUTTON_H))
        .style(theme::style::button::secondary()),
    );
    actions = actions.push(iced::widget::Space::new().width(Length::Fill));
    body = body.push(actions);

    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(PADDING_PAGE)
        .into()
}

pub(super) fn settings_title(fluent: &Fluent, category: SettingsCategory) -> String {
    let key = match category {
        SettingsCategory::General => Tr::General,
        SettingsCategory::Download => Tr::DownloadCategory,
        SettingsCategory::BitTorrent => Tr::BitTorrent,
        SettingsCategory::Ed2k => Tr::Ed2k,
        SettingsCategory::Network => Tr::Network,
        SettingsCategory::Advanced => Tr::Advanced,
    };
    fluent.get(key)
}

pub(super) fn level_label(fluent: &Fluent, level: &str) -> String {
    let key = match level {
        "trace" => Tr::LevelTrace,
        "debug" => Tr::LevelDebug,
        "info" => Tr::LevelInfo,
        "notice" => Tr::LevelNotice,
        "warn" => Tr::LevelWarn,
        "error" => Tr::LevelError,
        _ => return level.to_string(),
    };
    fluent.get(key)
}

pub(super) fn labeled_hint<'a>(hint: String) -> Element<'a, Message> {
    setting_row(
        String::new(),
        text(hint)
            .size(FONT_SMALL)
            .style(theme::style::text::secondary)
            .into(),
    )
}

pub(super) fn sub_items<'a>(
    children: impl IntoIterator<Item = Element<'a, Message>>,
) -> Element<'a, Message> {
    container(column(children).spacing(SPACE_SM))
        .width(Length::Fill)
        .padding(iced::Padding {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: SUB_ITEM_INDENT,
        })
        .into()
}

pub(super) fn labeled_number<'a, T>(
    label: String,
    value: T,
    bounds: impl std::ops::RangeBounds<T> + 'a,
    step: T,
    key: SettingKey,
) -> Element<'a, Message>
where
    T: num_traits::Num
        + num_traits::NumAssignOps
        + PartialOrd
        + std::fmt::Display
        + std::str::FromStr
        + Clone
        + Copy
        + num_traits::Bounded
        + ToSettingValue
        + 'static,
    <T as std::str::FromStr>::Err: std::fmt::Debug,
{
    setting_row(
        label,
        number_stepper(
            value,
            bounds,
            step,
            move |v| Message::Settings(SettingsMsg::SettingChanged(key, v.to_setting_value())),
            Length::Fixed(160.0),
        ),
    )
}

trait ToSettingValue {
    fn to_setting_value(self) -> SettingValue;
}

impl ToSettingValue for u16 {
    fn to_setting_value(self) -> SettingValue {
        SettingValue::Num(self as u64)
    }
}

impl ToSettingValue for u32 {
    fn to_setting_value(self) -> SettingValue {
        SettingValue::Num(self as u64)
    }
}

impl ToSettingValue for u64 {
    fn to_setting_value(self) -> SettingValue {
        SettingValue::Num(self)
    }
}

impl ToSettingValue for f64 {
    fn to_setting_value(self) -> SettingValue {
        SettingValue::NumF(self)
    }
}

pub(super) fn setting_row<'a>(
    label: String,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    row![]
        .push(
            container(text(label).size(FONT_MEDIUM))
                .width(Length::Fixed(200.0))
                .center_y(Length::Fixed(36.0)),
        )
        .push(control)
        .align_y(Alignment::Center)
        .into()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn labeled_port<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    label: String,
    value: u16,
    bounds: std::ops::RangeInclusive<u16>,
    key: SettingKey,
    kind: crate::port_guard::PortKind,
    port_status: &'a std::collections::HashMap<
        crate::port_guard::PortKind,
        (u16, crate::port_guard::PortStatus),
    >,
) -> Element<'a, Message> {
    let stepper = number_stepper(
        value,
        bounds,
        1,
        move |v| Message::Settings(SettingsMsg::SettingChanged(key, v.to_setting_value())),
        Length::Fixed(160.0),
    );
    let status = if port_status.get(&kind).map(|(p, _)| *p) == Some(value) {
        match port_status.get(&kind).map(|(_, s)| s) {
            Some(crate::port_guard::PortStatus::InUse) => Some(
                text(fluent.get(Tr::PortInUse))
                    .size(FONT_SMALL)
                    .color(theme::danger(theme)),
            ),
            Some(crate::port_guard::PortStatus::ConflictWith(other)) => {
                let mut args = std::collections::HashMap::new();
                args.insert(
                    std::borrow::Cow::from("other"),
                    fluent.get(other.tr()).into(),
                );
                Some(
                    text(fluent.get_args(Tr::PortConflictWith, &args))
                        .size(FONT_SMALL)
                        .color(theme::warning(theme)),
                )
            }
            Some(crate::port_guard::PortStatus::Available) => {
                let msg = if value == 0 {
                    Tr::PortAutoHint
                } else {
                    Tr::PortAvailable
                };
                Some(
                    text(fluent.get(msg))
                        .size(FONT_SMALL)
                        .style(theme::style::text::secondary),
                )
            }
            None => None,
        }
    } else {
        None
    };
    let control = match status {
        Some(s) => row![stepper, s]
            .spacing(SPACE_SM)
            .align_y(Alignment::Center)
            .into(),
        None => stepper,
    };
    setting_row(label, control)
}

pub(super) fn labeled_toggle<'a>(
    label: String,
    value: bool,
    key: SettingKey,
) -> Element<'a, Message> {
    setting_row(
        label,
        toggler(value)
            .on_toggle(move |v| {
                Message::Settings(SettingsMsg::SettingChanged(key, SettingValue::Bool(v)))
            })
            .width(Length::Fixed(50.0))
            .into(),
    )
}

pub(super) fn labeled_checkbox<'a>(
    label: String,
    value: bool,
    key: SettingKey,
) -> Element<'a, Message> {
    setting_row(
        label,
        checkbox(value)
            .on_toggle(move |v| {
                Message::Settings(SettingsMsg::SettingChanged(key, SettingValue::Bool(v)))
            })
            .into(),
    )
}

pub(super) fn labeled_text_input<'a>(
    label: String,
    value: &'a str,
    key: SettingKey,
    secure: bool,
    placeholder: &str,
) -> Element<'a, Message> {
    let mut input = theme::input_layout(
        text_input(placeholder, value)
            .on_input(move |s| {
                Message::Settings(SettingsMsg::SettingChanged(key, SettingValue::Text(s)))
            })
            .width(Length::Fill)
            .style(theme::style::input::standard),
    );
    if secure {
        input = input.secure(true);
    }
    setting_row(label, input.into())
}

pub(super) fn labeled_editor<'a>(
    label: String,
    content: &'a text_editor::Content,
    on_edit: fn(text_editor::Action) -> Message,
    placeholder: String,
    height: f32,
    target: CtxTarget,
) -> Element<'a, Message> {
    row![]
        .push(text(label).size(FONT_MEDIUM).width(Length::Fixed(200.0)))
        .push(
            mouse_area(theme::editor_layout(
                text_editor(content)
                    .placeholder(placeholder)
                    .on_action(on_edit)
                    .height(Length::Fixed(height))
                    .style(theme::style::text_editor::standard),
            ))
            .on_right_press(Message::CtxOpen(target)),
        )
        .align_y(Alignment::Start)
        .into()
}

pub(super) fn labeled_pick<'a, T>(
    fluent: &'a Fluent,
    label: String,
    options: Vec<Labeled<T>>,
    selected: Option<T>,
    on_select: impl Fn(Labeled<T>) -> Message + 'a,
) -> Element<'a, Message>
where
    T: PartialEq + Clone + 'static,
{
    let placeholder = fluent.get(Tr::SelectPlaceholder);
    let sel = selected.and_then(|s| options.iter().find(|o| o.value == s).cloned());
    setting_row(
        label,
        pick_list(options, sel, on_select)
            .placeholder(&placeholder)
            .text_size(FONT_MEDIUM)
            .padding(theme::INPUT_PADDING)
            .width(Length::Fixed(180.0))
            .style(theme::style::pick_list::standard)
            .menu_style(theme::style::pick_list::menu)
            .into(),
    )
}

#[derive(Debug, Clone, Copy)]
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

pub(super) fn speed_labeled_input<'a>(
    label: String,
    value_kb: u64,
    unit: SpeedUnit,
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

    setting_row(
        label,
        row![]
            .spacing(SPACE_LG)
            .push(number_stepper(
                display_val,
                0..=u64::MAX,
                step,
                on_value,
                Length::Fixed(160.0),
            ))
            .push(
                pick_list(unit_opts, sel, move |o| on_unit(o.value))
                    .text_size(FONT_MEDIUM)
                    .padding(theme::INPUT_PADDING)
                    .width(Length::Fixed(80.0))
                    .style(theme::style::pick_list::standard)
                    .menu_style(theme::style::pick_list::menu),
            )
            .align_y(Alignment::Center)
            .into(),
    )
}

pub(super) fn group_title<'a>(fluent: &'a Fluent, key: Tr, accent: Color) -> Element<'a, Message> {
    text(fluent.get(key)).size(FONT_TITLE).color(accent).into()
}

pub(super) fn time_pick_options() -> Vec<Labeled<String>> {
    (0..48)
        .map(|i| {
            let minutes = i * 30;
            let value = format!("{:02}:{:02}", minutes / 60, minutes % 60);
            Labeled {
                value: value.clone(),
                label: value,
            }
        })
        .collect()
}
