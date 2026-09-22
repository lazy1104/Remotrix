use std::collections::HashSet;
use std::time::Duration;

use iced::widget::text_editor;
use iced::Task;

use crate::app::{
    apply_settings, begin_close, changelog_fetch_task, check_updates, concat_changelog,
    continue_close_flow, dismiss_toast, mark_settings_dirty, open_path_in_manager, rebuild_theme,
    revert_apply_settings, send_download_aria2_update, send_system_notification, set_page,
    spawn_toast, start_tracker_fetch, Remotrix, UpdateDialogState,
};
use crate::config;
use crate::config::MAX_CUSTOM_COLOR_HISTORY;
use crate::engine::EngineCmd;
use crate::i18n::{Fluent, Tr};
use crate::message::{ConfirmAction, Message, PathPickerId, SettingKey, SettingValue, SettingsMsg};
use crate::port_guard::{check_port, port_value, PortKind};
use crate::ui::components::toast::{Toast, ToastGroup, ToastKind};
use crate::ui::theme;

fn refresh_port_status(state: &mut Remotrix, edited: PortKind) {
    let mut kinds = vec![edited];
    if edited.is_tcp() {
        for peer in [PortKind::Rpc, PortKind::ExtensionApi, PortKind::Ed2k] {
            if peer != edited && state.port_status.contains_key(&peer) {
                kinds.push(peer);
            }
        }
    }
    for kind in kinds {
        let port = port_value(&state.settings, kind);
        state
            .port_status
            .insert(kind, (port, check_port(&state.settings, kind)));
    }
}

pub(crate) fn handle(state: &mut Remotrix, msg: SettingsMsg) -> Task<Message> {
    match msg {
        SettingsMsg::SettingChanged(key, value) => {
            match key {
                SettingKey::MaxConcurrent => {
                    if let SettingValue::Num(n) = value {
                        state.settings.max_concurrent = n.max(1) as u32;
                    }
                }
                SettingKey::Split => {
                    if let SettingValue::Num(n) = value {
                        state.settings.split = n.max(1) as u16;
                    }
                }
                SettingKey::DownloadLimit => {
                    if let SettingValue::Num(n) = value {
                        state.settings.download_limit_kb = n;
                    }
                }
                SettingKey::UploadLimit => {
                    if let SettingValue::Num(n) = value {
                        state.settings.upload_limit_kb = n;
                    }
                }
                SettingKey::MaxConnectionPerServer => {
                    if let SettingValue::Num(n) = value {
                        state.settings.aria2.max_connection_per_server = n.max(1) as u32;
                    }
                }
                SettingKey::MinSplitSize => {
                    if let SettingValue::Num(n) = value {
                        state.settings.aria2.min_split_size_mb = n;
                    }
                }
                SettingKey::AutoFileRenaming => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.aria2.auto_file_renaming = b;
                    }
                }
                SettingKey::AllowOverwrite => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.aria2.allow_overwrite = b;
                    }
                }
                SettingKey::Continue => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.aria2.r#continue = b;
                    }
                }
                SettingKey::CheckIntegrity => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.aria2.check_integrity = b;
                    }
                }
                SettingKey::MaxDownloadLimit => {
                    if let SettingValue::Num(n) = value {
                        state.settings.aria2.max_download_limit_kb = n;
                    }
                }
                SettingKey::MaxUploadLimit => {
                    if let SettingValue::Num(n) = value {
                        state.settings.aria2.max_upload_limit_kb = n;
                    }
                }
                SettingKey::LowestSpeedLimit => {
                    if let SettingValue::Num(n) = value {
                        state.settings.aria2.lowest_speed_limit_kb = n;
                    }
                }
                SettingKey::ProxyServer => {
                    if let SettingValue::Text(s) = value {
                        state.settings.aria2.proxy_server = s;
                    }
                }
                SettingKey::ProxyUsername => {
                    if let SettingValue::Text(s) = value {
                        state.settings.aria2.proxy_username = s;
                    }
                }
                SettingKey::ProxyPassword => {
                    if let SettingValue::Text(s) = value {
                        state.settings.aria2.proxy_password = s;
                    }
                }
                SettingKey::MaxTries => {
                    if let SettingValue::Num(n) = value {
                        state.settings.aria2.max_tries = n as u32;
                    }
                }
                SettingKey::RetryWait => {
                    if let SettingValue::Num(n) = value {
                        state.settings.aria2.retry_wait = n as u32;
                    }
                }
                SettingKey::ConnectTimeout => {
                    if let SettingValue::Num(n) = value {
                        state.settings.aria2.connect_timeout = n as u32;
                    }
                }
                SettingKey::TrackerAutoSync => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.tracker.auto_sync = b;
                    }
                }
                SettingKey::TrackerSyncInterval => {
                    if let SettingValue::Num(n) = value {
                        state.settings.tracker.sync_interval_hours = n as u32;
                    }
                }
                SettingKey::AutoUpdateEnabled => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.update.enabled = b;
                    }
                }
                SettingKey::UpdateCheckInterval => {
                    if let SettingValue::Num(n) = value {
                        state.settings.update.interval_hours = n as u32;
                    }
                }
                SettingKey::UpdateScope => {
                    if let SettingValue::Text(s) = value {
                        if let Some(scope) = crate::config::UpdateScope::from_str(&s) {
                            state.settings.update.scope = scope;
                        }
                    }
                }
                SettingKey::SilentUpdateScope => {
                    if let SettingValue::Text(s) = value {
                        if let Some(scope) = crate::config::SilentUpdateScope::from_str(&s) {
                            state.settings.update.silent_update_scope = scope;
                        }
                    }
                }
                SettingKey::BetaChannel => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.update.beta_channel = b;
                    }
                }
                SettingKey::SeedRatio => {
                    if let SettingValue::NumF(n) = value {
                        state.settings.aria2.seed_ratio = n.max(0.0);
                    }
                }
                SettingKey::SeedTime => {
                    if let SettingValue::Num(n) = value {
                        state.settings.aria2.seed_time = n as u32;
                    }
                }
                SettingKey::EnableDht => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.aria2.enable_dht = b;
                    }
                }
                SettingKey::BtRequireCrypto => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.aria2.bt_require_crypto = b;
                    }
                }
                SettingKey::BtEnableLpd => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.aria2.bt_enable_lpd = b;
                    }
                }
                SettingKey::EnablePeerExchange => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.aria2.enable_peer_exchange = b;
                    }
                }
                SettingKey::BtAutoDownload => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.aria2.bt_auto_download = b;
                    }
                }
                SettingKey::FileAllocation => {
                    if let SettingValue::Text(s) = value {
                        state.settings.aria2.file_allocation = s;
                    }
                }
                SettingKey::DiskCache => {
                    if let SettingValue::Num(n) = value {
                        state.settings.aria2.disk_cache_mb = n;
                    }
                }
                SettingKey::EnableProxy => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.aria2.proxy_enabled = b;
                    }
                }
                SettingKey::NavToTasksAfterAdd => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.nav_to_tasks_after_add = b;
                    }
                }
                SettingKey::CloseToTray => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.close_to_tray = b;
                    }
                }
                SettingKey::AutoStart => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.autostart_enabled = b;
                    }
                }
                SettingKey::StartHiddenOnAutostart => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.start_hidden_on_autostart = b;
                    }
                }
                SettingKey::DeleteTorrentAfterComplete => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.delete_torrent_after_complete = b;
                    }
                }
                SettingKey::CleanupCompletedOnClose => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.cleanup_completed_on_close = b;
                    }
                }
                SettingKey::RemoveTaskIfFilesMissing => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.remove_task_if_files_missing = b;
                    }
                }
                SettingKey::NotificationDownloadComplete => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.notifications.download_complete = b;
                    }
                }
                SettingKey::NotificationDownloadError => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.notifications.download_error = b;
                    }
                }
                SettingKey::NotificationEngineDegraded => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.notifications.engine_degraded = b;
                    }
                }
                SettingKey::NotificationDownloadAdded => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.notifications.download_added = b;
                    }
                }
                SettingKey::PreventSleep => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.prevent_sleep = b;
                    }
                }
                SettingKey::ExtensionApiEnabled => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.extension.enabled = b;
                    }
                }
                SettingKey::ExtensionApiPort => {
                    if let SettingValue::Num(n) = value {
                        let port = n.clamp(
                            crate::config::EXTENSION_API_MIN_PORT as u64,
                            crate::config::EXTENSION_API_MAX_PORT as u64,
                        ) as u16;
                        state.settings.extension.port = port;
                        refresh_port_status(state, PortKind::ExtensionApi);
                    }
                }
                SettingKey::ExtensionApiSecret => {
                    if let SettingValue::Text(s) = value {
                        state.settings.extension.secret = s;
                    }
                }
                SettingKey::ExtensionAutoSubmit => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.extension.auto_submit = b;
                    }
                }
                SettingKey::DetectClipboardOnStart => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.detect_clipboard_on_start = b;
                    }
                }
                SettingKey::ClipboardHttp => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.clipboard_types.http = b;
                    }
                }
                SettingKey::ClipboardFtp => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.clipboard_types.ftp = b;
                    }
                }
                SettingKey::ClipboardMagnet => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.clipboard_types.magnet = b;
                    }
                }
                SettingKey::ClipboardEd2k => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.clipboard_types.ed2k = b;
                    }
                }
                SettingKey::ClipboardThunder => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.clipboard_types.thunder = b;
                    }
                }
                SettingKey::ClipboardBtInfohash => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.clipboard_types.bt_infohash = b;
                    }
                }
                SettingKey::ClipboardWebpageFilter => {
                    if let SettingValue::Text(s) = value {
                        if let Some(mode) = crate::clipboard_watch::WebpageFilterMode::from_str(&s)
                        {
                            state.settings.webpage_filter = mode;
                        }
                    }
                }
                SettingKey::Ed2kServer => {
                    if let SettingValue::Text(s) = value {
                        state.settings.aria2.ed2k_server = s;
                    }
                }
                SettingKey::Ed2kListenPort => {
                    if let SettingValue::Num(n) = value {
                        let port = n as u16;
                        state.settings.aria2.ed2k_listen_port = port;
                        refresh_port_status(state, PortKind::Ed2k);
                    }
                }
                SettingKey::Ed2kUdpListenPort => {
                    if let SettingValue::Num(n) = value {
                        let port = n as u16;
                        state.settings.aria2.ed2k_udp_listen_port = port;
                        refresh_port_status(state, PortKind::Ed2kUdp);
                    }
                }
                SettingKey::RpcListenPort => {
                    if let SettingValue::Num(n) = value {
                        let port = n as u16;
                        state.settings.aria2.rpc_listen_port = port;
                        refresh_port_status(state, PortKind::Rpc);
                    }
                }
                SettingKey::Ed2kUploadSlots => {
                    if let SettingValue::Num(n) = value {
                        state.settings.aria2.ed2k_upload_slots = n.max(1) as u16;
                    }
                }
                SettingKey::FollowMetalink => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.aria2.follow_metalink = b;
                    }
                }
                SettingKey::Ed2kServerMetUrl => {
                    if let SettingValue::Text(s) = value {
                        state.settings.aria2.ed2k_server_met_url = s;
                    }
                }
                SettingKey::Ed2kNodesDatUrl => {
                    if let SettingValue::Text(s) = value {
                        state.settings.aria2.ed2k_nodes_dat_url = s;
                    }
                }
                SettingKey::Ed2kBootstrapAutoSync => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.aria2.ed2k_bootstrap_auto_sync = b;
                    }
                }
                SettingKey::Ed2kBootstrapSyncInterval => {
                    if let SettingValue::Num(n) = value {
                        state.settings.aria2.ed2k_bootstrap_sync_interval_hours = n as u32;
                    }
                }
                SettingKey::Ed2kSearchKeyword
                | SettingKey::Ed2kSearchFileType
                | SettingKey::Ed2kSearchMinSources
                | SettingKey::Ed2kSearchTimeout => {
                    state.settings_ui.ed2k_search_state.update(key, value);
                }
                SettingKey::SpeedLimitScheduleEnabled => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.speed_limit_schedule.enabled = b;
                    }
                }
                SettingKey::ScheduleStart => {
                    if let SettingValue::Text(s) = value {
                        if crate::scheduler::parse_hhmm(&s).is_some() {
                            state.settings.speed_limit_schedule.start = s;
                        }
                    }
                }
                SettingKey::ScheduleEnd => {
                    if let SettingValue::Text(s) = value {
                        if crate::scheduler::parse_hhmm(&s).is_some() {
                            state.settings.speed_limit_schedule.end = s;
                        }
                    }
                }
                SettingKey::AppLogLevel => {
                    if let SettingValue::Text(s) = value {
                        state.settings.log.app_level = crate::logging::normalize_app_level(&s);
                    }
                }
                SettingKey::EngineLogLevel => {
                    if let SettingValue::Text(s) = value {
                        state.settings.log.engine_level =
                            crate::logging::normalize_engine_level(&s);
                    }
                }
            }
            mark_settings_dirty(state);
            Task::none()
        }
        SettingsMsg::ApplySettings => {
            apply_settings(state);
            Task::none()
        }
        SettingsMsg::ResetSettings => {
            revert_apply_settings(state);
            config::save(&state.settings);
            Task::none()
        }
        SettingsMsg::ClearLogs => {
            match crate::logging::clear_logs() {
                Ok(count) => {
                    tracing::info!(count, "ui: cleared log files");
                    let toast = Toast::new(ToastKind::Success, state.fluent.get(Tr::LogsCleared))
                        .group(ToastGroup::Logs)
                        .close_after(Some(Duration::from_secs(3)));
                    state.toasts.push(toast);
                }
                Err(e) => {
                    tracing::warn!(?e, "ui: clear log files failed");
                    let toast = Toast::new(ToastKind::Error, state.fluent.get(Tr::LogsClearFailed))
                        .group(ToastGroup::Logs)
                        .close_after(Some(Duration::from_secs(5)));
                    state.toasts.push(toast);
                }
            }
            Task::none()
        }
        SettingsMsg::ThemeModeChanged(mode) => {
            state.settings.theme_mode = mode;
            rebuild_theme(state);
            config::save(&state.settings);
            state.applied_settings.theme_mode = mode;
            Task::none()
        }
        SettingsMsg::SystemDarkChanged(_) => {
            if state.settings.theme_mode == theme::ThemeMode::System {
                rebuild_theme(state);
            }
            Task::none()
        }
        SettingsMsg::ThemeColorChanged(color) => {
            state.settings.theme_color = crate::ui::theme::color_to_hex(color);
            rebuild_theme(state);
            config::save(&state.settings);
            state.applied_settings.theme_color = state.settings.theme_color.clone();
            Task::none()
        }
        SettingsMsg::CustomColorPickerToggle => {
            if state.custom_color_picker_open {
                state.custom_color_picker_open = false;
            } else {
                state.settings_ui.custom_color_picker =
                    crate::ui::components::color_picker::CustomColorPickerUi::seed_from(
                        &state.settings.theme_color,
                    );
                state.custom_color_anchor = state.last_cursor;
                state.custom_color_picker_open = true;
            }
            Task::none()
        }
        SettingsMsg::CustomColorHsvChanged(hsv) => {
            let picker = &mut state.settings_ui.custom_color_picker;
            picker.hsv = hsv;
            picker.hex_input =
                theme::color_to_hex(crate::ui::components::color_picker::hsv_to_color(&hsv));
            picker.hex_valid = true;
            Task::none()
        }
        SettingsMsg::CustomColorHexChanged(raw) => {
            let picker = &mut state.settings_ui.custom_color_picker;
            picker.hex_input = raw;
            picker.hex_valid = theme::color_from_hex(&picker.hex_input).is_some();
            Task::none()
        }
        SettingsMsg::CustomColorApply => {
            let picker = &mut state.settings_ui.custom_color_picker;
            let Some(color) = theme::color_from_hex(&picker.hex_input) else {
                return Task::none();
            };
            let canonical = theme::color_to_hex(color);
            if let Some(pos) = state
                .settings
                .custom_color_history
                .iter()
                .position(|h| h.eq_ignore_ascii_case(&canonical))
            {
                state.settings.custom_color_history.remove(pos);
            }
            state.settings.custom_color_history.insert(0, canonical);
            if state.settings.custom_color_history.len() > MAX_CUSTOM_COLOR_HISTORY {
                state
                    .settings
                    .custom_color_history
                    .truncate(MAX_CUSTOM_COLOR_HISTORY);
            }
            state.applied_settings.custom_color_history =
                state.settings.custom_color_history.clone();
            config::save(&state.settings);
            state.custom_color_picker_open = false;
            let msg = SettingsMsg::ThemeColorChanged(color);
            handle(state, msg)
        }
        SettingsMsg::CustomColorCancel => {
            state.custom_color_picker_open = false;
            Task::none()
        }
        SettingsMsg::CustomColorHistorySelect(hex) => {
            let Some(color) = theme::color_from_hex(&hex) else {
                return Task::none();
            };
            let mut hsv = crate::ui::components::color_picker::color_to_hsv(color);
            if hsv.val < 0.0001 {
                hsv.val = 1.0;
            }
            let picker = &mut state.settings_ui.custom_color_picker;
            picker.hsv = hsv;
            picker.hex_input = theme::color_to_hex(color);
            picker.hex_valid = true;
            Task::none()
        }
        SettingsMsg::LocaleChanged(locale) => {
            state.settings.locale = locale;
            state.fluent = Fluent::new(locale);
            crate::i18n::set_current_locale(locale);
            config::save(&state.settings);
            state.applied_settings.locale = locale;
            Task::none()
        }
        SettingsMsg::FontFamilyChanged(family) => {
            state.settings.font_family = family;
            state.settings_ui.font_picker.open = false;
            state.settings_ui.font_picker.query.clear();
            mark_settings_dirty(state);
            Task::none()
        }
        SettingsMsg::FontPickerToggle => {
            state.settings_ui.font_picker.open = !state.settings_ui.font_picker.open;
            if !state.settings_ui.font_picker.open {
                state.settings_ui.font_picker.query.clear();
            }
            Task::none()
        }
        SettingsMsg::FontPickerClose => {
            state.settings_ui.font_picker.open = false;
            state.settings_ui.font_picker.query.clear();
            Task::none()
        }
        SettingsMsg::FontPickerQueryChanged(query) => {
            state.settings_ui.font_picker.query = query;
            state.settings_ui.font_picker.open = true;
            Task::none()
        }
        SettingsMsg::RestartApp => {
            config::save(&state.settings);
            state.applied_settings = state.settings.clone();
            state.applied_font_family = state.settings.font_family.clone();
            state.settings_dirty = false;
            state.restart_pending = true;
            begin_close(state)
        }
        SettingsMsg::SpeedUnitChanged(key, unit) => {
            state.settings_ui.speed_units.insert(key, unit);
            Task::none()
        }
        SettingsMsg::Ed2kSearchSubmit => {
            let Some(cmd) = state.settings_ui.ed2k_search_state.build_cmd() else {
                return Task::none();
            };
            if state.handle.cmd_tx.send(cmd).is_err() {
                tracing::warn!("ui: ed2k search submit send failed");
            }
            Task::none()
        }
        SettingsMsg::Ed2kSearchCancel => {
            for gid in state.settings_ui.ed2k_search_state.cancel() {
                if state
                    .handle
                    .cmd_tx
                    .send(EngineCmd::Ed2kSearchCleanup { gid })
                    .is_err()
                {
                    tracing::warn!("ui: ed2k search cleanup send failed");
                }
            }
            Task::none()
        }
        SettingsMsg::Ed2kBootstrapSyncNow => {
            if state.settings_ui.syncing_bootstrap {
                return Task::none();
            }
            state.settings_ui.syncing_bootstrap = true;
            if state
                .handle
                .cmd_tx
                .send(EngineCmd::Ed2kBootstrapSyncNow)
                .is_err()
            {
                state.settings_ui.syncing_bootstrap = false;
                tracing::warn!("ui: ed2k bootstrap sync now send failed");
            }
            Task::none()
        }
        SettingsMsg::UaEditor(action) => {
            state.ua_editor.perform(action);
            state.settings.aria2.user_agent = state.ua_editor.text();
            mark_settings_dirty(state);
            Task::none()
        }
        SettingsMsg::BtTrackerEditor(action) => {
            state.bt_tracker_editor.perform(action);
            state.settings.aria2.bt_tracker = state.bt_tracker_editor.text();
            mark_settings_dirty(state);
            Task::none()
        }
        SettingsMsg::TrackerSourceToggled { source, enabled } => {
            if enabled {
                if !state.settings.tracker.sources.contains(&source) {
                    state.settings.tracker.sources.push(source);
                }
            } else {
                state.settings.tracker.sources.retain(|s| s != &source);
            }
            mark_settings_dirty(state);
            Task::none()
        }
        SettingsMsg::TrackerCustomInputChanged(v) => {
            state.settings_ui.custom_tracker_input = v;
            Task::none()
        }
        SettingsMsg::TrackerCustomAdd => {
            let input = state.settings_ui.custom_tracker_input.trim().to_string();
            if input.is_empty() {
                return Task::none();
            }
            let is_http = input.starts_with("http://") || input.starts_with("https://");
            if !is_http || reqwest::Url::parse(&input).is_err() {
                let toast = Toast::new(
                    ToastKind::Warning,
                    state.fluent.get(Tr::BtTrackerSourceInvalidUrl),
                )
                .group(ToastGroup::Tracker)
                .close_after(Some(Duration::from_secs(4)));
                state.toasts.push(toast);
                return Task::none();
            }
            if !state.settings.tracker.custom_urls.contains(&input) {
                state.settings.tracker.custom_urls.push(input.clone());
            }
            if !state.settings.tracker.sources.contains(&input) {
                state.settings.tracker.sources.push(input);
            }
            state.settings_ui.custom_tracker_input.clear();
            mark_settings_dirty(state);
            Task::none()
        }
        SettingsMsg::TrackerCustomRemove(url) => {
            state.settings.tracker.custom_urls.retain(|u| u != &url);
            state.settings.tracker.sources.retain(|u| u != &url);
            mark_settings_dirty(state);
            Task::none()
        }
        SettingsMsg::SyncTrackers => {
            if state.settings_ui.syncing_trackers {
                return Task::none();
            }
            let urls = state.settings.tracker.sources.clone();
            if urls.is_empty() {
                let toast = Toast::new(
                    ToastKind::Warning,
                    state.fluent.get(Tr::BtTrackerSelectSource),
                )
                .group(ToastGroup::Tracker)
                .close_after(Some(Duration::from_secs(4)));
                state.toasts.push(toast);
                return Task::none();
            }
            start_tracker_fetch(state, urls)
        }
        SettingsMsg::TrackersSynced { fetched, failures } => {
            if !state.settings_ui.syncing_trackers {
                if let Some(id) = state.settings_ui.tracker_sync_toast_id.take() {
                    dismiss_toast(state, id);
                }
                return Task::none();
            }
            state.settings_ui.syncing_trackers = false;
            if let Some(id) = state.settings_ui.tracker_sync_toast_id.take() {
                dismiss_toast(state, id);
            }
            let ok = fetched.len();
            let failed = failures.len();
            let total = ok + failed;
            let mut lines: Vec<String> = Vec::new();
            let mut seen = HashSet::new();
            for body in &fetched {
                for line in crate::trackers::parse_lines(body) {
                    if seen.insert(line.clone()) {
                        lines.push(line);
                    }
                }
            }
            if lines.is_empty() && !failures.is_empty() {
                let toast = Toast::new(ToastKind::Error, state.fluent.get(Tr::BtTrackerSyncFailed))
                    .group(ToastGroup::Tracker)
                    .close_after(Some(Duration::from_secs(5)));
                state.toasts.push(toast);
                return Task::none();
            }
            let text = crate::trackers::to_lines(&lines.join("\n"));
            let count = crate::trackers::count(&text);
            state.bt_tracker_editor = text_editor::Content::with_text(&text);
            state.settings.aria2.bt_tracker = text;
            state.applied_settings.aria2.bt_tracker = state.settings.aria2.bt_tracker.clone();
            let now_ms = chrono::Local::now().timestamp_millis();
            state.settings.tracker.last_sync_time = Some(now_ms);
            state.applied_settings.tracker.last_sync_time = Some(now_ms);
            config::save(&state.applied_settings);
            let opts = state.settings.effective_task_options();
            if state
                .handle
                .cmd_tx
                .send(EngineCmd::ApplyAria2Options { options: opts })
                .is_err()
            {
                tracing::warn!("ui: apply aria2 options cmd send failed");
            }
            let msg = if failures.is_empty() {
                let mut args = std::collections::HashMap::new();
                args.insert(std::borrow::Cow::from("count"), (count as i64).into());
                state.fluent.get_args(Tr::BtTrackerSyncSucceed, &args)
            } else {
                let mut args = std::collections::HashMap::new();
                args.insert(std::borrow::Cow::from("ok"), (ok as i64).into());
                args.insert(std::borrow::Cow::from("total"), (total as i64).into());
                args.insert(std::borrow::Cow::from("failed"), (failed as i64).into());
                state.fluent.get_args(Tr::BtTrackerSyncPartial, &args)
            };
            let toast = Toast::new(
                if failures.is_empty() {
                    ToastKind::Success
                } else {
                    ToastKind::Warning
                },
                msg,
            )
            .group(ToastGroup::Tracker)
            .close_after(Some(Duration::from_secs(4)));
            state.toasts.push(toast);
            Task::none()
        }
        SettingsMsg::TrackerSyncTimedOut => {
            if !state.settings_ui.syncing_trackers {
                return Task::none();
            }
            state.settings_ui.syncing_trackers = false;
            if let Some(id) = state.settings_ui.tracker_sync_toast_id.take() {
                dismiss_toast(state, id);
            }
            let toast = Toast::new(ToastKind::Error, state.fluent.get(Tr::BtTrackerSyncTimeout))
                .group(ToastGroup::Tracker)
                .close_after(Some(Duration::from_secs(5)));
            state.toasts.push(toast);
            Task::none()
        }
        SettingsMsg::CheckTrackerAutoSync { startup } => {
            if state.settings_ui.syncing_trackers {
                return Task::none();
            }
            if state.settings.aria2.bt_tracker != state.applied_settings.aria2.bt_tracker {
                return Task::none();
            }
            let now_ms = chrono::Local::now().timestamp_millis();
            if !crate::trackers::sync_due(
                state.settings.tracker.auto_sync,
                state.settings.tracker.sync_interval_hours,
                state.settings.tracker.last_sync_time,
                startup,
                now_ms,
            ) {
                return Task::none();
            }
            let urls = state.settings.tracker.sources.clone();
            if urls.is_empty() {
                return Task::none();
            }
            start_tracker_fetch(state, urls)
        }
        SettingsMsg::CheckUpdatesNow => check_updates(state, false, true),
        SettingsMsg::CheckAutoUpdate { startup } => check_updates(state, startup, false),
        SettingsMsg::UpdateDialogTab(i) => {
            if let Some(dialog) = &mut state.update_dialog {
                dialog.active_tab = i;
            }
            Task::none()
        }
        SettingsMsg::RetryChangelog(tab) => {
            if let Some(dialog) = &mut state.update_dialog {
                if let Some(changelog) = dialog.changelogs.get_mut(tab) {
                    changelog.loading = true;
                    changelog.failed = false;
                }
            }
            changelog_fetch_task(state, tab)
        }
        SettingsMsg::UpdateChangelogLoaded { tab, releases } => {
            if let Some(dialog) = &mut state.update_dialog {
                if let Some(changelog) = dialog.changelogs.get_mut(tab) {
                    changelog.loading = false;
                    match releases {
                        Ok(rels) => {
                            changelog.failed = false;
                            let text = concat_changelog(&rels);
                            changelog.md = iced::widget::markdown::Content::parse(&text);
                            if let Some(offer) = dialog.offers.get_mut(tab) {
                                offer.changelog = text;
                            }
                        }
                        Err(e) => {
                            changelog.failed = true;
                            spawn_toast(
                                state,
                                ToastGroup::General,
                                ToastKind::Error,
                                format!("{}: {e}", state.fluent.get(Tr::UpdateFailed)),
                                Some(Duration::from_secs(6)),
                                true,
                            );
                        }
                    }
                }
            }
            Task::none()
        }
        SettingsMsg::UpdateDialogCancel => {
            state.update_dialog_anim.begin_exit();
            Task::none()
        }
        SettingsMsg::AppUpdateProgress { downloaded, total } => {
            state.engine_ui.app_update_progress = Some((downloaded, total));
            Task::none()
        }
        SettingsMsg::AppUpdateFailed { error } => {
            state.app_update_in_flight = false;
            state.engine_ui.app_update_progress = None;
            spawn_toast(
                state,
                ToastGroup::General,
                ToastKind::Error,
                format!("{}: {error}", state.fluent.get(Tr::UpdateFailed)),
                Some(Duration::from_secs(6)),
                true,
            );
            Task::none()
        }
        SettingsMsg::AppUpdateReady { outcome } => {
            state.app_update_in_flight = false;
            state.engine_ui.app_update_progress = None;
            let label = state.fluent.get(Tr::UpdateApply);
            let toast = Toast::new(
                ToastKind::Normal,
                state.fluent.get(Tr::UpdateReadyClickToApply),
            )
            .group(ToastGroup::General)
            .close_after(None)
            .show_close()
            .action(
                crate::ui::components::toast::ToastAction::ApplyAppUpdate(outcome),
                label,
            );
            state.toasts.push(toast);
            Task::none()
        }
        SettingsMsg::ApplyAppUpdate { outcome } => {
            let path = outcome.path.clone();
            let kind = outcome.kind;
            match kind {
                crate::app_updater::InstallKind::AppImage => {
                    let Some(path) = path.as_ref() else {
                        return Task::none();
                    };
                    if let Err(e) = crate::app_updater::apply_after_download(kind, path) {
                        spawn_toast(
                            state,
                            ToastGroup::General,
                            ToastKind::Error,
                            format!("{}: {e}", state.fluent.get(Tr::UpdateFailed)),
                            Some(Duration::from_secs(6)),
                            true,
                        );
                        return Task::none();
                    }
                    spawn_toast(
                        state,
                        ToastGroup::General,
                        ToastKind::Success,
                        state.fluent.get(Tr::UpdateAppimageReplaced),
                        Some(Duration::from_secs(5)),
                        false,
                    );
                    state.restart_pending = true;
                    return begin_close(state);
                }
                crate::app_updater::InstallKind::WindowsSetup => {
                    let Some(path) = path.as_ref() else {
                        return Task::none();
                    };
                    let apply_result = crate::app_updater::apply_after_download(kind, path);
                    spawn_toast(
                        state,
                        ToastGroup::General,
                        ToastKind::Success,
                        state.fluent.get(Tr::UpdateRunInstaller),
                        Some(Duration::from_secs(5)),
                        false,
                    );
                    if apply_result.is_ok() {
                        crate::app_updater::remove_app_update_file(path);
                    }
                    return Task::none();
                }
                crate::app_updater::InstallKind::Deb => {
                    let path = path.unwrap_or_default();
                    let download_dir = path
                        .parent()
                        .map(std::path::Path::to_path_buf)
                        .unwrap_or_default();
                    let mut args = std::collections::HashMap::new();
                    args.insert(
                        std::borrow::Cow::from("path"),
                        std::borrow::Cow::from(path.to_string_lossy().into_owned()).into(),
                    );
                    spawn_toast(
                        state,
                        ToastGroup::General,
                        ToastKind::Success,
                        state.fluent.get_args(Tr::UpdatePackageDownloaded, &args),
                        Some(Duration::from_secs(5)),
                        false,
                    );
                    if state.settings.notifications.download_complete {
                        let title = state.fluent.get(Tr::UpdatePackageDownloadedTitle);
                        let body = state.fluent.get_args(Tr::UpdatePackageDownloaded, &args);
                        let path_clone = path.clone();
                        send_system_notification(
                            state,
                            title,
                            body,
                            vec![
                                (
                                    state.fluent.get(Tr::Open),
                                    crate::notify::NotifyAction::OpenFile(path_clone),
                                ),
                                (
                                    state.fluent.get(Tr::Locate),
                                    crate::notify::NotifyAction::RevealDir(download_dir.clone()),
                                ),
                            ],
                            crate::notify::NotifyAction::OpenFile(path.clone()),
                        );
                    }
                    crate::app_updater::remove_app_update_file(&path);
                    return open_path_in_manager(download_dir);
                }
            }
        }
        SettingsMsg::UpdateResult {
            offers,
            silent_applied,
            errors,
            pending_restart_engine,
            pending_app_update,
        } => {
            state.engine_ui.update_check_in_flight = false;
            let checked_any = state.settings.update.scope.covers("aria2-next")
                || state.settings.update.scope.covers("remotrix");
            for e in errors.iter() {
                spawn_toast(
                    state,
                    ToastGroup::General,
                    ToastKind::Error,
                    format!("{}: {e}", state.fluent.get(Tr::UpdateFailed)),
                    Some(Duration::from_secs(6)),
                    true,
                );
            }
            let mut silent_tasks: Vec<Task<Message>> = Vec::new();
            for silent in &silent_applied {
                match silent.component {
                    crate::ui::update_dialog::UpdateComponent::Aria2 => {
                        send_download_aria2_update(state, silent, true);
                    }
                    crate::ui::update_dialog::UpdateComponent::App => {
                        if !state.app_update_in_flight {
                            silent_tasks.push(kick_off_app_update(state, silent, false));
                        }
                    }
                }
            }
            if pending_restart_engine {
                push_pending_engine_toast(state);
            }
            if let Some(outcome) = pending_app_update.clone() {
                push_pending_app_toast(state, outcome);
            }
            if !offers.is_empty() {
                let offer_count = offers.len();
                let changelogs = offers
                    .iter()
                    .map(|_| crate::ui::update_dialog::ChangelogState {
                        md: iced::widget::markdown::Content::default(),
                        loading: true,
                        failed: false,
                    })
                    .collect();
                state.update_dialog = Some(UpdateDialogState {
                    changelogs,
                    offers,
                    active_tab: 0,
                });
                state.update_dialog_anim.open();
                if state.window.hidden_to_tray {
                    send_system_notification(
                        state,
                        state.fluent.get(Tr::UpdateTrayNotifyTitle),
                        state.fluent.get(Tr::UpdateTrayNotifyBody),
                        vec![],
                        crate::notify::NotifyAction::ActivateWindow,
                    );
                    tracing::info!(offers = offer_count, "tray update notification sent");
                }
                let mut tasks = silent_tasks;
                for tab in 0..state.update_dialog.as_ref().unwrap().offers.len() {
                    tasks.push(changelog_fetch_task(state, tab));
                }
                return Task::batch(tasks);
            }
            if !silent_tasks.is_empty() {
                return Task::batch(silent_tasks);
            }
            if checked_any
                && errors.is_empty()
                && !pending_restart_engine
                && pending_app_update.is_none()
            {
                spawn_toast(
                    state,
                    ToastGroup::General,
                    ToastKind::Success,
                    state.fluent.get(Tr::UpToDate),
                    Some(Duration::from_secs(3)),
                    false,
                );
            }
            Task::none()
        }
        SettingsMsg::UpdateDialogApply => {
            if state.app_update_in_flight {
                return Task::none();
            }
            if state.update_dialog_anim.is_dismissing() {
                return Task::none();
            }
            let Some(offers) = state
                .update_dialog
                .as_ref()
                .map(|d| d.offers.clone())
                .filter(|o| !o.is_empty())
            else {
                return Task::none();
            };
            state.update_dialog_anim.begin_exit();
            for offer in offers {
                match offer.component {
                    crate::ui::update_dialog::UpdateComponent::Aria2 => {
                        send_download_aria2_update(state, &offer, false);
                    }
                    crate::ui::update_dialog::UpdateComponent::App => {
                        if state.app_update_in_flight {
                            continue;
                        }
                        return kick_off_app_update(state, &offer, true);
                    }
                }
            }
            Task::none()
        }
        SettingsMsg::ToggleScheduleDaysMenu => {
            state.settings_ui.schedule_days_menu_open = !state.settings_ui.schedule_days_menu_open;
            Task::none()
        }
        SettingsMsg::RestoreDefaultPath(id) => {
            match id {
                PathPickerId::CustomAria2Dir => {
                    state.settings.paths.aria2_bin_dir = None;
                    state.applied_settings.paths.aria2_bin_dir = None;
                }
                PathPickerId::CustomAppDataDir => {
                    state.settings.paths.app_data_dir = None;
                    state.applied_settings.paths.app_data_dir = None;
                }
                PathPickerId::CustomLogDir => {
                    state.settings.paths.log_dir = None;
                    state.applied_settings.paths.log_dir = None;
                }
                _ => {}
            }
            state.restart_pending = true;
            mark_settings_dirty(state);
            Task::none()
        }
        SettingsMsg::ScheduleDayToggled { day, enabled } => {
            let weekdays = &mut state.settings.speed_limit_schedule.weekdays;
            if enabled {
                if !weekdays.contains(&day) {
                    weekdays.push(day);
                    weekdays.sort_unstable();
                }
            } else {
                weekdays.retain(|d| *d != day);
            }
            mark_settings_dirty(state);
            Task::none()
        }
        SettingsMsg::ApplyAndLeaveSettings => {
            if state.confirm_anim.is_dismissing() {
                return Task::none();
            }
            if let Some(ConfirmAction::LeaveSettings { target }) = state.confirm.as_ref() {
                let target = *target;
                state.confirm_anim.begin_exit();
                apply_settings(state);
                set_page(state, target);
            }
            Task::none()
        }
        SettingsMsg::DiscardAndLeaveSettings => {
            if state.confirm_anim.is_dismissing() {
                return Task::none();
            }
            if let Some(ConfirmAction::LeaveSettings { target }) = state.confirm.as_ref() {
                let target = *target;
                state.confirm_anim.begin_exit();
                revert_apply_settings(state);
                config::save(&state.settings);
                set_page(state, target);
            }
            Task::none()
        }
        SettingsMsg::ApplyAndClose => {
            if state.confirm_anim.is_dismissing() {
                return Task::none();
            }
            if matches!(state.confirm, Some(ConfirmAction::UnsavedOnClose)) {
                state.confirm_anim.begin_exit();
                apply_settings(state);
                return continue_close_flow(state);
            }
            Task::none()
        }
        SettingsMsg::DiscardAndClose => {
            if state.confirm_anim.is_dismissing() {
                return Task::none();
            }
            if matches!(state.confirm, Some(ConfirmAction::UnsavedOnClose)) {
                state.confirm_anim.begin_exit();
                revert_apply_settings(state);
                config::save(&state.settings);
                return continue_close_flow(state);
            }
            Task::none()
        }
        SettingsMsg::CheckPendingUpdates => {
            if state.settings.update.scope.covers("aria2-next") {
                if let Some(dir) = crate::config::aria2_bin_dir() {
                    if crate::aria2_fetcher::pending_update(&dir).is_some() {
                        push_pending_engine_toast(state);
                    }
                }
            }
            if state.settings.update.scope.covers("remotrix") {
                let outcome =
                    crate::app_updater::find_pending_app_update(Some(&state.settings.download_dir));
                if let Some(outcome) = outcome {
                    push_pending_app_toast(state, outcome);
                }
            }
            Task::none()
        }
    }
}

fn push_pending_engine_toast(state: &mut Remotrix) {
    let already = state.toasts.toasts.iter().any(|t| {
        matches!(
            t.action,
            Some(crate::ui::components::toast::ToastAction::RestartEngine)
        )
    });
    if already {
        return;
    }
    let label = state.fluent.get(Tr::RestartEngine);
    let toast = Toast::new(ToastKind::Normal, state.fluent.get(Tr::UpdateEngineRestart))
        .group(ToastGroup::Engine)
        .close_after(None)
        .show_close()
        .action(
            crate::ui::components::toast::ToastAction::RestartEngine,
            label,
        );
    state.toasts.push(toast);
}

fn push_pending_app_toast(state: &mut Remotrix, outcome: crate::app_updater::AppUpdateOutcome) {
    let already = state.toasts.toasts.iter().any(|t| {
        matches!(
            &t.action,
            Some(crate::ui::components::toast::ToastAction::ApplyAppUpdate(_))
        )
    });
    if already {
        return;
    }
    let label = state.fluent.get(Tr::UpdateApply);
    let toast = Toast::new(
        ToastKind::Normal,
        state.fluent.get(Tr::UpdateReadyClickToApply),
    )
    .group(ToastGroup::General)
    .close_after(None)
    .show_close()
    .action(
        crate::ui::components::toast::ToastAction::ApplyAppUpdate(outcome),
        label,
    );
    state.toasts.push(toast);
}

fn kick_off_app_update(
    state: &mut Remotrix,
    offer: &crate::ui::update_dialog::UpdateOffer,
    show_downloading_toast: bool,
) -> Task<Message> {
    let kind = crate::app_updater::detect_install_kind();
    let asset_name = offer.asset_name.clone();
    let dest = match crate::app_updater::app_update_dest(
        kind,
        &asset_name,
        Some(&state.settings.download_dir),
    ) {
        Ok(d) => d,
        Err(e) => {
            spawn_toast(
                state,
                ToastGroup::General,
                ToastKind::Error,
                format!("{}: {e}", state.fluent.get(Tr::UpdateFailed)),
                Some(Duration::from_secs(6)),
                true,
            );
            return Task::none();
        }
    };
    state.app_update_in_flight = true;
    state.engine_ui.app_update_progress = None;
    if show_downloading_toast {
        spawn_toast(
            state,
            ToastGroup::General,
            ToastKind::Normal,
            state.fluent.get(Tr::UpdateDownloading),
            None,
            true,
        );
    }
    let version = offer.latest.clone();
    let download_url = offer.download_url.clone();
    let offer_sha256 = offer.sha256.clone();
    let asset_name_owned = asset_name.clone();
    let proxy = state.settings.aria2.all_proxy_value();
    let dest_for_task = dest.clone();
    let dest_for_msg = dest;
    Task::perform(
        crate::download::perform_app_update_download(
            download_url,
            dest_for_task,
            proxy,
            offer_sha256,
            version,
            kind,
            Some(asset_name_owned),
            dest_for_msg,
        ),
        |res| match res {
            Ok(outcome) => Message::Settings(SettingsMsg::AppUpdateReady { outcome }),
            Err(error) => Message::Settings(SettingsMsg::AppUpdateFailed { error }),
        },
    )
}
