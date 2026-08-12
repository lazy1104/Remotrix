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
use crate::engine::EngineCmd;
use crate::i18n::{Fluent, Tr};
use crate::message::{ConfirmAction, Message, SettingKey, SettingValue, SettingsMsg};
use crate::port_guard::{check_port, port_value, PortKind};
use crate::ui::components::toast::{Toast, ToastGroup, ToastKind};

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
                SettingKey::Aria2SilentUpdate => {
                    if let SettingValue::Bool(b) = value {
                        state.settings.update.aria2_silent_update = b;
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
        SettingsMsg::ThemeColorChanged(color) => {
            state.settings.theme_color = crate::ui::theme::color_to_hex(color);
            rebuild_theme(state);
            config::save(&state.settings);
            state.applied_settings.theme_color = state.settings.theme_color.clone();
            Task::none()
        }
        SettingsMsg::LocaleChanged(locale) => {
            state.settings.locale = locale;
            state.fluent = Fluent::new(locale);
            config::save(&state.settings);
            state.applied_settings.locale = locale;
            Task::none()
        }
        SettingsMsg::FontFamilyChanged(family) => {
            state.settings.font_family = family;
            mark_settings_dirty(state);
            Task::none()
        }
        SettingsMsg::RestartApp => {
            state.restart_pending = true;
            begin_close(state)
        }
        SettingsMsg::SpeedUnitChanged(key, unit) => {
            state.settings_ui.speed_units.insert(key, unit);
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
        SettingsMsg::UpdateDownloadStarted(result) => {
            state.app_update_in_flight = false;
            match result {
                Ok(outcome) => match outcome.kind {
                    crate::app_updater::InstallKind::AppImage => {
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
                        spawn_toast(
                            state,
                            ToastGroup::General,
                            ToastKind::Success,
                            state.fluent.get(Tr::UpdateRunInstaller),
                            Some(Duration::from_secs(5)),
                            false,
                        );
                    }
                    crate::app_updater::InstallKind::Deb => {
                        let path = outcome.path.unwrap_or_default();
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
                                        crate::notify::NotifyAction::RevealDir(
                                            download_dir.clone(),
                                        ),
                                    ),
                                ],
                                crate::notify::NotifyAction::OpenFile(path),
                            );
                        }
                        return open_path_in_manager(download_dir);
                    }
                },
                Err(e) => {
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
            Task::none()
        }
        SettingsMsg::UpdateResult {
            offers,
            silent_applied,
            errors,
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
            for silent in &silent_applied {
                send_download_aria2_update(state, silent, true);
            }
            if !offers.is_empty() {
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
                let mut tasks = Vec::new();
                for tab in 0..state.update_dialog.as_ref().unwrap().offers.len() {
                    tasks.push(changelog_fetch_task(state, tab));
                }
                return Task::batch(tasks);
            } else if checked_any && errors.is_empty() {
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
                        state.app_update_in_flight = true;
                        let kind = crate::app_updater::detect_install_kind();
                        let version = offer.latest.clone();
                        let download_url = offer.download_url.clone();
                        let asset_name = offer.asset_name.clone();
                        let offer_sha256 = offer.sha256.clone();
                        let download_dir = state.settings.download_dir.clone();
                        spawn_toast(
                            state,
                            ToastGroup::General,
                            ToastKind::Normal,
                            state.fluent.get(Tr::UpdateDownloading),
                            None,
                            true,
                        );
                        let _ = state.handle.cmd_tx.send(EngineCmd::DownloadAppUpdate {
                            kind,
                            version,
                            url: download_url,
                            asset_name,
                            sha256: offer_sha256,
                            download_dir,
                        });
                    }
                }
            }
            Task::none()
        }
        SettingsMsg::ToggleScheduleDaysMenu => {
            state.settings_ui.schedule_days_menu_open = !state.settings_ui.schedule_days_menu_open;
            Task::none()
        }
        SettingsMsg::ReadOnlyHover { path, hovered } => {
            if hovered {
                state.settings_ui.readonly_hovered.insert(path);
            } else {
                state.settings_ui.readonly_hovered.remove(&path);
            }
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
    }
}
