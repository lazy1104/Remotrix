use iced::Task;

use crate::app::Remotrix;
use crate::message::Message;

pub(crate) mod add;
pub(crate) mod engine;
pub(crate) mod misc;
pub(crate) mod nav;
pub(crate) mod settings;
pub(crate) mod task;
pub(crate) mod window;

pub(crate) fn dispatch(state: &mut Remotrix, message: Message) -> Task<Message> {
    let task = match message {
        Message::Nav(msg) => nav::handle(state, msg),
        Message::Add(msg) => add::handle(state, msg),
        Message::Task(msg) => task::handle(state, msg),
        Message::Settings(msg) => settings::handle(state, msg),
        Message::Engine(msg) => engine::handle(state, msg),
        Message::Window(msg) => window::handle(state, msg),
        Message::Sort(msg) => misc::handle_sort(state, msg),
        Message::Dialog(msg) => misc::handle_dialog(state, msg),
        Message::Toast(msg) => misc::handle_toast(state, msg),
        Message::Tray(msg) => misc::handle_tray(state, msg),
        Message::Extension(msg) => misc::handle_extension(state, msg),
        Message::Shutdown(msg) => misc::handle_shutdown(state, msg),
        Message::CopyText(s) => crate::app::copy_to_clipboard(state, s),
        Message::CtxOpen(target) => {
            state.ctx_open = Some((target, state.last_cursor));
            iced::clipboard::read().map(Message::CtxClipboardRead)
        }
        Message::CtxClipboardRead(text) => {
            if let Some((target, position)) = state.ctx_open.take() {
                state.ctx_menu = Some(crate::app::CtxMenuState {
                    target,
                    position,
                    clipboard: text,
                });
            }
            Task::none()
        }
        Message::CtxClose => {
            state.ctx_menu = None;
            state.ctx_open = None;
            Task::none()
        }
        Message::CtxCopy(selected) => {
            state.ctx_menu = None;
            state.ctx_open = None;
            crate::app::copy_to_clipboard(state, selected)
        }
        Message::CtxPaste(target, text) => {
            state.ctx_menu = None;
            state.ctx_open = None;
            Task::done(crate::app::ctx_paste_message(state, target, text))
        }
        Message::CursorMoved(pos) => {
            state.last_cursor = pos;
            Task::none()
        }
        Message::OpenLink(url) => {
            let scheme = url.split(':').next().unwrap_or_default();
            if !matches!(scheme, "http" | "https") {
                Task::none()
            } else {
                Task::perform(
                    async move {
                        let _ = open::that(&url);
                    },
                    |_| Message::Noop,
                )
            }
        }
        Message::OpenFile(path) => Task::perform(
            async move {
                let _ = open::that(&path);
            },
            |_| Message::Noop,
        ),
        Message::RevealDir(path) => crate::app::open_path_in_manager(path),
        Message::ShowRequested => misc::handle_show_requested(state),
        Message::ActivateWindow => misc::handle_activate_window(state),
        Message::ProgressAnim(gid, event) => {
            if let Some(anim) = state.progress_anim.get_mut(&gid) {
                anim.update(event);
            }
            Task::none()
        }
        Message::CardAnim(gid, event) => {
            let done = if let Some(anim) = state.card_anim.get_mut(&gid) {
                anim.update(event);
                !anim.is_animating()
            } else {
                true
            };
            if done && state.pending_removals.contains(&gid) {
                crate::app::finalize_task_removal(state, &gid);
            }
            Task::none()
        }
        Message::HudAnim(event) => {
            state.hud_anim.update(event);
            Task::none()
        }
        Message::PillAnim(event) => {
            state.filter_pill.update(event);
            Task::none()
        }
        Message::AddDialogAnim(event) => {
            state.add_dialog_anim.update(event);
            if state.add_dialog_anim.completed_dismiss() {
                state.add_dialog.close();
            }
            Task::none()
        }
        Message::AboutDialogAnim(event) => {
            state.about_dialog_anim.update(event);
            if state.about_dialog_anim.completed_dismiss() {
                state.about_dialog_visible = false;
            }
            Task::none()
        }
        Message::DetailsAnim(event) => {
            state.details_anim.update(event);
            if state.details_anim.completed_dismiss() {
                state.details.close();
            }
            Task::none()
        }
        Message::ConfirmAnim(event) => {
            state.confirm_anim.update(event);
            if state.confirm_anim.completed_dismiss() {
                state.confirm = None;
            }
            Task::none()
        }
        Message::UpdateDialogAnim(event) => {
            state.update_dialog_anim.update(event);
            if state.update_dialog_anim.completed_dismiss() {
                state.update_dialog = None;
            }
            Task::none()
        }
        Message::CloseDialogAnim(event) => {
            if let Some(anim) = &mut state.window.close_dialog_anim {
                anim.update(event);
                if state.window.close_dialog_dismissing && !anim.is_animating() {
                    state.window.show_close_dialog = false;
                    state.window.close_dialog_anim = None;
                    state.window.close_dialog_dismissing = false;
                }
            }
            Task::none()
        }
        Message::Noop => Task::none(),
    };
    state
        .hud_anim
        .set_target(if state.tracking.active_count > 0 {
            1.0
        } else {
            0.0
        });
    task
}
