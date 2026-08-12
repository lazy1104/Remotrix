use iced::Task;

#[cfg(target_os = "windows")]
use crate::app::RESIZE_QUIET_MS;
use crate::app::{
    begin_close, finalize_close, flush_dirty, hide_to_tray, open_close_dialog, read_clipboard,
    refresh_tray, spawn_restart_if_pending, spawn_toast, sync_geometry_to_settings, Remotrix,
};
use crate::engine::EngineCmd;
use crate::i18n::Tr;
use crate::message::{CloseDialogChoice, ConfirmAction, Message, WindowCmd, WindowMsg};
use crate::ui::components::toast::{ToastGroup, ToastKind};
use std::time::Duration;
#[cfg(target_os = "windows")]
use std::time::Instant;

pub(crate) fn handle(state: &mut Remotrix, msg: WindowMsg) -> Task<Message> {
    match msg {
        WindowMsg::DroppedFileParsed(payload) => {
            if state.window.show_close_dialog
                || state.about_dialog_visible
                || state.confirm.is_some()
                || state.update_dialog.is_some()
            {
                return Task::none();
            }
            let Some(payload) = payload else {
                spawn_toast(
                    state,
                    ToastGroup::Task,
                    ToastKind::Warning,
                    state.fluent.get(Tr::NoDownloadableContent),
                    Some(Duration::from_secs(4)),
                    false,
                );
                return Task::none();
            };
            if let crate::clipboard_watch::ClipboardPayload::Torrent(ref path) = payload {
                if !crate::ui::components::torrent_upload::is_valid_torrent_file(path) {
                    spawn_toast(
                        state,
                        ToastGroup::Task,
                        ToastKind::Warning,
                        state.fluent.get(Tr::InvalidTorrent),
                        Some(Duration::from_secs(4)),
                        false,
                    );
                    return Task::none();
                }
            }
            if state.add_dialog.is_visible() {
                state.add_dialog.apply_payload(payload);
                return Task::none();
            }
            state.add_dialog.open_with(
                state.settings.download_dir.clone(),
                state.settings.split,
                payload,
            );
            state.add_dialog_anim.open();
            spawn_toast(
                state,
                ToastGroup::Task,
                ToastKind::Normal,
                state.fluent.get(Tr::DropDetected),
                Some(Duration::from_secs(3)),
                false,
            );
            Task::none()
        }
        WindowMsg::WindowResized(size) => {
            state.window.last_resize = Some(size);
            #[cfg(target_os = "windows")]
            {
                state.window.resizing = true;
                state.window.resize_quiet =
                    Some(Instant::now() + Duration::from_millis(RESIZE_QUIET_MS));
            }
            #[cfg(not(target_os = "windows"))]
            {
                state.window.geometry_dirty = true;
            }
            Task::none()
        }
        WindowMsg::WindowOpened(id) => {
            if state.window.window_id.is_none() {
                state.window.window_id = Some(id);
                if state.window.hidden_to_tray && !state.tray.enabled() {
                    state.window.hidden_to_tray = false;
                    refresh_tray(state);
                    return iced::window::set_mode::<Message>(id, iced::window::Mode::Windowed)
                        .chain(read_clipboard(state));
                }
                return read_clipboard(state);
            }
            Task::none()
        }
        WindowMsg::WindowFocused(id) => {
            if state.window.window_id.is_none() || state.window.window_id == Some(id) {
                return read_clipboard(state);
            }
            Task::none()
        }
        WindowMsg::ClipboardRead(content) => {
            let Some(text) = content else {
                return Task::none();
            };
            let trimmed = text.trim().to_string();
            let prefs = state.settings.clipboard_types;
            Task::perform(
                async move {
                    let payload = crate::clipboard_watch::parse_clipboard(&trimmed, prefs);
                    let hash = crate::clipboard_watch::payload_hash(&payload);
                    (payload, hash)
                },
                |(payload, hash)| Message::Window(WindowMsg::ClipboardParsed(payload, hash)),
            )
        }
        WindowMsg::ClipboardParsed(payload, hash) => {
            let Some(payload) = payload else {
                return Task::none();
            };
            if state.add_dialog.is_visible() {
                return Task::none();
            }
            if hash == state.settings.last_clipboard_hash {
                return Task::none();
            }
            state.settings.last_clipboard_hash = hash.clone();
            state.applied_settings.last_clipboard_hash = hash;
            crate::config::save(&state.settings);
            state.add_dialog.open_with(
                state.settings.download_dir.clone(),
                state.settings.split,
                payload,
            );
            state.add_dialog_anim.open();
            spawn_toast(
                state,
                ToastGroup::Task,
                ToastKind::Normal,
                state.fluent.get(Tr::ClipboardDetected),
                Some(Duration::from_secs(3)),
                false,
            );
            Task::none()
        }
        WindowMsg::DragWindow => {
            if let Some(id) = state.window.window_id {
                return iced::window::drag::<Message>(id);
            }
            Task::none()
        }
        WindowMsg::ResizeWindow(direction) => {
            if let Some(id) = state.window.window_id {
                return iced::window::drag_resize::<Message>(id, direction);
            }
            Task::none()
        }
        WindowMsg::WindowAction(cmd) => {
            if let Some(id) = state.window.window_id {
                return match cmd {
                    WindowCmd::Minimize => iced::window::minimize::<Message>(id, true),
                    WindowCmd::ToggleMaximize => {
                        state.window.maximized = !state.window.maximized;
                        iced::window::toggle_maximize::<Message>(id)
                    }
                };
            }
            Task::none()
        }
        WindowMsg::CloseRequested => {
            if state.window.closing {
                return Task::none();
            }
            if state.settings_dirty {
                state.confirm = Some(ConfirmAction::UnsavedOnClose);
                state.confirm_anim.open();
                return Task::none();
            }
            if state.settings.close_to_tray && state.tray.enabled() {
                return hide_to_tray(state);
            }
            state.window.show_close_dialog = true;
            open_close_dialog(state);
            Task::none()
        }
        WindowMsg::HideToTray => {
            if state.window.closing {
                return Task::none();
            }
            hide_to_tray(state)
        }
        WindowMsg::CloseDialog(choice) => match choice {
            CloseDialogChoice::Close => {
                state.window.show_close_dialog = false;
                state.window.close_dialog_anim = None;
                state.window.close_dialog_dismissing = false;
                begin_close(state)
            }
            CloseDialogChoice::Cancel => {
                if let Some(anim) = &mut state.window.close_dialog_anim {
                    anim.set_target(0.0);
                    state.window.close_dialog_dismissing = true;
                }
                Task::none()
            }
        },
        WindowMsg::CloseDialogTrayPrefChanged(b) => {
            state.settings.close_to_tray = b;
            state.applied_settings.close_to_tray = b;
            crate::config::save(&state.settings);
            Task::none()
        }
        WindowMsg::ShutdownRequested => begin_close(state),
        WindowMsg::ShutdownTimeout => {
            if state.window.closing {
                tracing::warn!("engine did not stop in time, closing anyway");
                if state.handle.cmd_tx.send(EngineCmd::ForceKill).is_err() {
                    tracing::warn!("force-kill cmd send failed");
                }
            }
            finalize_close(state)
        }
        WindowMsg::PersistWindowGeometry => {
            if state.window.geometry_dirty {
                if let Some(id) = state.window.window_id {
                    return iced::window::is_maximized(id)
                        .then(|max| Task::done(Message::Window(WindowMsg::WindowMaximized(max))));
                }
            }
            Task::none()
        }
        WindowMsg::WindowMaximized(max) => {
            state.window.maximized = max;
            if let Some(s) = state.window.last_resize {
                if !max {
                    state.window.window_size = s;
                }
                state.window.last_resize = None;
            }
            sync_geometry_to_settings(state);
            crate::config::save(&state.settings);
            state.window.geometry_dirty = false;
            if state.window.pending_close {
                state.window.pending_close = false;
                spawn_restart_if_pending(state);
                if let Some(id) = state.window.window_id {
                    return iced::window::close::<Message>(id);
                }
            }
            Task::none()
        }
        WindowMsg::FlushDirty => {
            flush_dirty(state);
            Task::none()
        }
        #[cfg(target_os = "windows")]
        WindowMsg::ResizeTick => {
            if !state.window.resizing {
                return Task::none();
            }
            let settled = state
                .window
                .resize_quiet
                .map(|d| Instant::now() >= d)
                .unwrap_or(false);
            if let Some(s) = state.window.last_resize {
                state.window.window_size = s;
            }
            if settled {
                state.window.resizing = false;
                state.window.resize_quiet = None;
                state.window.geometry_dirty = true;
            }
            Task::none()
        }
        #[cfg(not(target_os = "windows"))]
        WindowMsg::ResizeTick => Task::none(),
    }
}
