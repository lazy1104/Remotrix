use std::time::Duration;

use iced::Task;

use crate::app::{
    dismiss_toast, hide_to_tray, mark_settings_dirty, open_add_dialog, refresh_tray,
    reset_shutdown_card, restore_window_from_tray, restore_window_from_tray_wayland,
    send_system_notification, set_page, spawn_toast, Remotrix,
};
use crate::i18n::Tr;
use crate::message::{
    ConfirmAction, DialogMsg, ExtensionMsg, Message, Page, SettingKey, ShutdownMsg, SortMsg,
    SortOrder, ToastMsg, TrayMsg,
};
use crate::ui::components::toast::{ToastGroup, ToastKind};

pub(crate) fn handle_sort(state: &mut Remotrix, msg: SortMsg) -> Task<Message> {
    match msg {
        SortMsg::SortSelected(field) => {
            state.sort_field = field;
            Task::none()
        }
        SortMsg::ToggleSortMenu => {
            state.sort_menu_open = !state.sort_menu_open;
            Task::none()
        }
        SortMsg::CloseSortMenu => {
            state.sort_menu_open = false;
            Task::none()
        }
        SortMsg::ToggleSortOrder => {
            state.sort_order = match state.sort_order {
                SortOrder::Asc => SortOrder::Desc,
                SortOrder::Desc => SortOrder::Asc,
            };
            Task::none()
        }
        SortMsg::SearchChanged(query) => {
            state.search_query = query;
            Task::none()
        }
    }
}

pub(crate) fn handle_dialog(state: &mut Remotrix, msg: DialogMsg) -> Task<Message> {
    match msg {
        DialogMsg::OpenAbout => {
            state.about_dialog_visible = true;
            state.about_dialog_anim.open();
            Task::none()
        }
        DialogMsg::CloseAbout => {
            state.about_dialog_anim.begin_exit();
            Task::none()
        }
        DialogMsg::RequestConfirm(action) => {
            state.confirm = Some(action);
            state.confirm_anim.open();
            Task::none()
        }
        DialogMsg::ConfirmCancel => {
            if state.confirm_anim.is_dismissing() {
                return Task::none();
            }
            if matches!(state.confirm, Some(ConfirmAction::Shutdown { .. })) {
                reset_shutdown_card(state);
            }
            state.confirm_anim.begin_exit();
            Task::none()
        }
        DialogMsg::OpenSpeedLimitPopover => {
            state.speed_limit_popover_open = !state.speed_limit_popover_open;
            Task::none()
        }
        DialogMsg::CloseSpeedLimitPopover => {
            state.speed_limit_popover_open = false;
            crate::app::flush_speed_limit_debounce(state);
            Task::none()
        }
        DialogMsg::SpeedLimitChanged(key, v) => {
            match key {
                SettingKey::DownloadLimit => state.settings.download_limit_kb = v,
                SettingKey::UploadLimit => state.settings.upload_limit_kb = v,
                _ => {}
            }
            state.applied_settings = state.settings.clone();
            crate::app::schedule_speed_limit_deadline(state, key);
            Task::none()
        }
        DialogMsg::SpeedLimitUnitChanged(key, unit) => {
            state.settings_ui.speed_units.insert(key, unit);
            Task::none()
        }
    }
}

pub(crate) fn handle_toast(state: &mut Remotrix, msg: ToastMsg) -> Task<Message> {
    match msg {
        ToastMsg::DismissToast(id) => {
            dismiss_toast(state, id);
            Task::none()
        }
        ToastMsg::ToastHovered(id) => {
            state.toasts.hover(id);
            Task::none()
        }
        ToastMsg::ToastUnhovered(id) => {
            state.toasts.unhover(id);
            Task::none()
        }
        ToastMsg::ToastTick => {
            state.toasts.tick();
            Task::none()
        }
    }
}

pub(crate) fn handle_tray(state: &mut Remotrix, msg: TrayMsg) -> Task<Message> {
    match msg {
        TrayMsg::ClickShow => restore_window_from_tray_wayland(state),
        TrayMsg::ToggleWindow => {
            if state.window.hidden_to_tray {
                restore_window_from_tray_wayland(state)
            } else {
                hide_to_tray(state)
            }
        }
        TrayMsg::OpenAddDialog => {
            tracing::info!(
                window_id = state.window.window_id.is_some(),
                "tray new download"
            );
            open_add_dialog(state);
            let attention = state
                .window
                .window_id
                .map(|id| {
                    iced::window::request_user_attention(
                        id,
                        Some(iced::window::UserAttention::Critical),
                    )
                })
                .unwrap_or_else(Task::none);
            restore_window_from_tray_wayland(state).chain(attention)
        }
        TrayMsg::OpenSettings => {
            tracing::info!(
                window_id = state.window.window_id.is_some(),
                "tray open settings"
            );
            state.settings_ui.download_picker.close_history();
            set_page(state, Page::Settings);
            let attention = state
                .window
                .window_id
                .map(|id| {
                    iced::window::request_user_attention(
                        id,
                        Some(iced::window::UserAttention::Critical),
                    )
                })
                .unwrap_or_else(Task::none);
            restore_window_from_tray_wayland(state).chain(attention)
        }
        TrayMsg::WatchdogReaddRequested => {
            tracing::info!("tray watchdog: re-adding tray icon");
            state.tray.recreate(state.tray_tx.clone());
            refresh_tray(state);
            Task::none()
        }
    }
}

pub(crate) fn handle_extension(state: &mut Remotrix, msg: ExtensionMsg) -> Task<Message> {
    match msg {
        ExtensionMsg::GenerateSecret => {
            state.settings.extension.secret = crate::extension_api::generate_secret();
            mark_settings_dirty(state);
            Task::none()
        }
        ExtensionMsg::ShowAddDialog(download) => {
            state.add_dialog.open_external(
                state.settings.download_dir.clone(),
                state.settings.split,
                download,
            );
            Task::none()
        }
        ExtensionMsg::ServerRestarted { ok } => {
            let (msg, kind) = if ok {
                (state.fluent.get(Tr::ExtensionRestarted), ToastKind::Success)
            } else {
                (
                    state.fluent.get(Tr::ExtensionRestartFailed),
                    ToastKind::Error,
                )
            };
            spawn_toast(
                state,
                ToastGroup::Engine,
                kind,
                msg,
                Some(Duration::from_secs(3)),
                false,
            );
            Task::none()
        }
    }
}

pub(crate) fn handle_shutdown(state: &mut Remotrix, msg: ShutdownMsg) -> Task<Message> {
    crate::app::handle_shutdown(state, msg)
}

pub(crate) fn handle_show_requested(state: &mut Remotrix) -> Task<Message> {
    if state.window.closing {
        return Task::none();
    }
    if state.window.hidden_to_tray {
        return restore_window_from_tray(state);
    }
    let title = state.fluent.get(Tr::AppRunningTitle);
    let body = state.fluent.get(Tr::AppRunningBody);
    send_system_notification(
        state,
        title,
        body,
        vec![],
        crate::notify::NotifyAction::ActivateWindow,
    );
    Task::none()
}

pub(crate) fn handle_activate_window(state: &mut Remotrix) -> Task<Message> {
    let attention = state
        .window
        .window_id
        .map(|id| {
            iced::window::request_user_attention(id, Some(iced::window::UserAttention::Critical))
        })
        .unwrap_or_else(Task::none);
    restore_window_from_tray(state).chain(attention)
}
