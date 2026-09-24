use iced::alignment::{Horizontal, Vertical};
use iced::widget::{column, container, row, stack};
use iced::{Element, Length, Padding, Vector};

use crate::app::{ctx_value, Remotrix};
use crate::message::{CtxTarget, Message, Page, SettingsMsg, ShutdownMsg, TaskFilter};
use crate::task::{DownloadTask, TaskStatus};
use crate::ui::category_bar::Counts;
use crate::ui::components::ctx_menu;
use crate::ui::dims::{CATEGORY_W, PADDING_CARD, SIDEBAR_W};
use crate::ui::theme;

const SHUTDOWN_CARD_ANCHOR: f32 = 112.0;
const SPEED_POPOVER_GAP: f32 = 8.0;
const COLOR_POPOVER_WIDTH: f32 = 280.0;
const COLOR_POPOVER_GAP: f32 = 8.0;

pub fn view(state: &Remotrix) -> Element<'_, Message> {
    let counts = Counts {
        all: state.tasks.len(),
        downloading: state
            .tasks
            .values()
            .filter(|t| {
                matches!(
                    t.status,
                    TaskStatus::Active | TaskStatus::Waiting | TaskStatus::Paused
                )
            })
            .count(),
        completed: state
            .tasks
            .values()
            .filter(|t| matches!(t.status, TaskStatus::Completed))
            .count(),
        failed: state
            .tasks
            .values()
            .filter(|t| matches!(t.status, TaskStatus::Error))
            .count(),
    };

    let t = &state.theme;
    #[cfg(target_os = "windows")]
    if state.window.resizing {
        return resize_placeholder_view(state);
    }
    let titlebar = crate::ui::title_bar::view(t, state.window.maximized);
    let left_col = crate::ui::sidebar::view(&state.fluent, t, state.page);

    let mid_bg = crate::ui::category_bar::background(t);
    let mid_content_inner = crate::ui::category_bar::content(
        &state.fluent,
        t,
        state.page,
        state.task_filter,
        state.settings_cat,
        &counts,
        &state.filter_pill,
    );

    let mid_col: Element<'_, Message> = iced::widget::Stack::new()
        .push(mid_content_inner)
        .push_under(mid_bg)
        .width(Length::Fill)
        .height(Length::Fill)
        .into();

    let right_col_inner: Element<'_, Message> = match state.page {
        Page::Tasks => {
            let query = state.search_query.trim().to_lowercase();
            let filtered: Vec<&DownloadTask> = state
                .task_order
                .iter()
                .filter_map(|gid| state.tasks.get(gid))
                .filter(|t| match state.task_filter {
                    TaskFilter::All => true,
                    TaskFilter::Downloading => matches!(
                        t.status,
                        TaskStatus::Active | TaskStatus::Waiting | TaskStatus::Paused
                    ),
                    TaskFilter::Completed => matches!(t.status, TaskStatus::Completed),
                    TaskFilter::Failed => matches!(t.status, TaskStatus::Error),
                })
                .filter(|t| {
                    query.is_empty()
                        || t.name.to_lowercase().contains(&query)
                        || t.url.to_lowercase().contains(&query)
                })
                .collect();
            let sorted = crate::ui::sort::sort_tasks(&filtered, state.sort_field, state.sort_order);
            crate::ui::task_list::view(
                &state.fluent,
                t,
                &sorted,
                !state.tasks.is_empty(),
                state.task_filter,
                state.sort_field,
                state.sort_order,
                state.sort_menu_open,
                &state.search_query,
                &state.progress_anim,
                &state.card_anim,
                &state.input_cursors,
                state.hovered_task_gid.as_deref(),
            )
        }
        Page::Settings => {
            let ctx = crate::ui::settings_page::SettingsPageContext {
                fluent: &state.fluent,
                theme: t,
                settings: &state.settings,
                settings_ui: &state.settings_ui,
                category: state.settings_cat,
                applied_settings: &state.applied_settings,
                settings_dirty: state.settings_dirty,
                engine_restart_pending: state.restart.engine_restart_pending,
                engine_restart_in_progress: state.restart.engine_restart_in_progress,
                aria2_version: state.engine_ui.aria2_version.as_deref(),
                aria2_status: state
                    .engine_ui
                    .aria2_status
                    .as_ref()
                    .map(|(s, m)| (s.as_str(), m.as_str())),
                aria2_fetch_error: state.engine_ui.aria2_fetch_error.as_deref(),
                update_check_in_flight: state.engine_ui.update_check_in_flight,
                aria2_download_version: state.engine_ui.aria2_downloading_version.as_deref(),
                aria2_download_progress: state.engine_ui.aria2_download_progress,
                ua_editor: &state.ua_editor,
                bt_tracker_editor: &state.bt_tracker_editor,
                path_history: &state.settings.path_history,
                font_restart_required: state.settings.font_family != state.applied_font_family,
                ctx_mirrors: &state.input_cursors,
                port_status: &state.port_status,
            };
            crate::ui::settings_page::view(&ctx)
        }
    };

    let right_col: Element<'_, Message> = right_col_inner;

    let content = row![]
        .push(
            container(left_col)
                .width(Length::Fixed(SIDEBAR_W))
                .height(Length::Fill),
        )
        .push(
            container(mid_col)
                .width(Length::Fixed(CATEGORY_W))
                .height(Length::Fill),
        )
        .push(
            container(right_col)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill);

    let body = column![]
        .push(titlebar)
        .push(content)
        .width(Length::Fill)
        .height(Length::Fill);

    let framed: iced::Element<'_, Message> = if state.window.maximized {
        let base = container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::style::base_background);
        #[allow(clippy::useless_conversion)]
        {
            iced::widget::opaque(base).into()
        }
    } else {
        let base = container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::style::base_background);
        let hairline: iced::Element<'_, Message> = container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::style::window_border)
            .into();
        let border_opacity = *state.border_anim.value();
        let animated_bar: iced::Element<'_, Message> = if border_opacity > 0.0 {
            crate::ui::border_bar::view(t, border_opacity)
        } else {
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(Length::Fixed(crate::ui::border_bar::HEIGHT))
                .into()
        };
        let animated_layer = crate::ui::animation::animation(&state.border_anim, animated_bar)
            .on_update(Message::BorderAnim);
        stack![
            iced::widget::opaque(base),
            crate::ui::resize_frame::view(),
            hairline,
            animated_layer,
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    };
    let (dl, up) = if state.tracking.active_count > 0 {
        state.global_speed.unwrap_or((0, 0))
    } else {
        (0, 0)
    };
    let hud_overlay = container(crate::ui::components::speed_hud::view(
        t,
        dl,
        up,
        &state.hud_anim,
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Right)
    .align_y(Vertical::Bottom)
    .padding(Padding {
        top: 0.0,
        right: 16.0,
        bottom: 20.0,
        left: 0.0,
    });
    let base_layer: iced::Element<'_, Message> = stack![framed, hud_overlay]
        .width(Length::Fill)
        .height(Length::Fill)
        .into();

    let add_layer: iced::Element<'_, Message> = if state.add_dialog.is_visible() {
        let content = crate::ui::add_dialog::view(
            &state.fluent,
            t,
            &state.add_dialog,
            &state.settings.path_history,
            state.add_dialog_anim.value(),
            &state.input_cursors,
        );
        crate::ui::components::dialog::overlay(
            crate::ui::animation::animation(state.add_dialog_anim.anim(), content)
                .on_update(Message::AddDialogAnim),
            state.add_dialog_anim.value(),
        )
    } else {
        iced::widget::Space::new().into()
    };

    let about_layer: iced::Element<'_, Message> = if state.about_dialog_visible {
        let content = crate::ui::about_dialog::view(
            &state.fluent,
            t,
            state.engine_ui.aria2_version.as_deref(),
            state.about_dialog_anim.value(),
        );
        crate::ui::components::dialog::overlay(
            crate::ui::animation::animation(state.about_dialog_anim.anim(), content)
                .on_update(Message::AboutDialogAnim),
            state.about_dialog_anim.value(),
        )
    } else {
        iced::widget::Space::new().into()
    };

    let close_layer: iced::Element<'_, Message> = if state.window.show_close_dialog {
        if let Some(anim) = &state.window.close_dialog_anim {
            let content = crate::ui::close_dialog::view(
                &state.fluent,
                t,
                state.tray.enabled(),
                state.settings.close_to_tray,
                *anim.value(),
            );
            crate::ui::components::dialog::overlay(
                crate::ui::animation::animation(anim, content).on_update(Message::CloseDialogAnim),
                *anim.value(),
            )
        } else {
            iced::widget::Space::new().into()
        }
    } else {
        iced::widget::Space::new().into()
    };

    let details_layer: iced::Element<'_, Message> = if state.details.is_visible() {
        let task = state
            .details
            .gid
            .as_deref()
            .and_then(|g| state.tasks.get(g));
        let content = crate::ui::details_dialog::view(
            &state.fluent,
            t,
            task,
            &state.details,
            state.details_anim.value(),
            &state.input_cursors,
            &state.progress_anim,
        );
        crate::ui::components::dialog::overlay(
            crate::ui::animation::animation(state.details_anim.anim(), content)
                .on_update(Message::DetailsAnim),
            state.details_anim.value(),
        )
    } else {
        iced::widget::Space::new().into()
    };

    let confirm_layer: iced::Element<'_, Message> = if let Some(action) = &state.confirm {
        let content =
            crate::ui::confirm_dialog::view(&state.fluent, t, action, state.confirm_anim.value());
        crate::ui::components::dialog::overlay(
            crate::ui::animation::animation(state.confirm_anim.anim(), content)
                .on_update(Message::ConfirmAnim),
            state.confirm_anim.value(),
        )
    } else {
        iced::widget::Space::new().into()
    };

    let update_layer: iced::Element<'_, Message> = if let Some(dialog) = &state.update_dialog {
        let content = crate::ui::update_dialog::view(
            &state.fluent,
            t,
            &dialog.offers,
            &dialog.changelogs,
            dialog.active_tab,
            state.update_dialog_anim.value(),
        );
        crate::ui::components::dialog::overlay(
            crate::ui::animation::animation(state.update_dialog_anim.anim(), content)
                .on_update(Message::UpdateDialogAnim),
            state.update_dialog_anim.value(),
        )
    } else {
        iced::widget::Space::new().into()
    };

    let drop_overlay_layer: iced::Element<'_, Message> = if state.drop_hover
        && !(state.window.show_close_dialog
            || state.about_dialog_visible
            || state.confirm.is_some()
            || state.update_dialog.is_some())
    {
        crate::ui::components::drop_overlay::view(&state.fluent, t)
    } else {
        iced::widget::Space::new().into()
    };

    let toast_layer: iced::Element<'_, Message> = if !state.toasts.toasts.is_empty() {
        crate::ui::components::toast::view(t, &state.toasts.toasts)
    } else {
        iced::widget::Space::new().into()
    };

    let ctx_layer: iced::Element<'_, Message> = if let Some(menu) = &state.ctx_menu {
        let selected: Option<String> = match menu.target {
            CtxTarget::AddUrl => state.add_dialog.url_editor.selection(),
            CtxTarget::SettingsUa => state.ua_editor.selection(),
            CtxTarget::SettingsBtTracker => state.bt_tracker_editor.selection(),
            t => state.input_cursors.get(&t).and_then(|c| {
                c.borrow().selection.map(|(a, b)| {
                    iced::widget::text_input::Value::new(ctx_value(state, t))
                        .select(a, b)
                        .to_string()
                })
            }),
        };
        let selected = if ctx_menu::is_secure_target(menu.target) {
            None
        } else {
            selected
        };
        let position = menu.position;
        let menu_el = ctx_menu::menu(&state.fluent, selected, menu.clipboard.clone(), menu.target);
        crate::ui::components::popover::popover(
            menu_el,
            move |bounds, viewport| {
                let px = position
                    .x
                    .clamp(0.0, (viewport.width - bounds.width).max(0.0));
                let py = position
                    .y
                    .clamp(0.0, (viewport.height - bounds.height).max(0.0));
                Vector::new(px - bounds.x, py - bounds.y)
            },
            Some(Message::CtxClose),
        )
    } else {
        iced::widget::Space::new().into()
    };

    let shutdown_layer: iced::Element<'_, Message> = if state.shutdown.card_open {
        let card = crate::ui::components::shutdown_popover::view(&state.fluent, t, &state.shutdown);
        crate::ui::components::popover::popover(
            card,
            move |bounds, viewport| {
                let x = SIDEBAR_W + 8.0;
                let y = (viewport.height - SHUTDOWN_CARD_ANCHOR)
                    .clamp(0.0, (viewport.height - bounds.height).max(0.0));
                Vector::new(x - bounds.x, y - bounds.y)
            },
            Some(Message::Shutdown(ShutdownMsg::CloseCard)),
        )
    } else {
        iced::widget::Space::new().into()
    };

    let speed_popover_layer: iced::Element<'_, Message> = if state.speed_limit_popover_open {
        let card_body = crate::ui::components::speed_popover::view(&state.fluent, state);
        let card = container(card_body)
            .padding(PADDING_CARD)
            .style(theme::style::subtle);
        crate::ui::components::popover::popover(
            card,
            move |bounds, viewport| {
                let x = (viewport.width - bounds.width - 16.0)
                    .clamp(0.0, (viewport.width - bounds.width).max(0.0));
                let y = (viewport.height
                    - 20.0
                    - crate::ui::components::speed_hud::HUD_SIZE
                    - SPEED_POPOVER_GAP
                    - bounds.height)
                    .clamp(0.0, (viewport.height - bounds.height).max(0.0));
                Vector::new(x - bounds.x, y - bounds.y)
            },
            Some(Message::Dialog(
                crate::message::DialogMsg::CloseSpeedLimitPopover,
            )),
        )
    } else {
        iced::widget::Space::new().into()
    };

    let color_popover_layer: iced::Element<'_, Message> = if state.custom_color_picker_open {
        let ui = &state.settings_ui.custom_color_picker;
        let on_hex = |s: String| SettingsMsg::CustomColorHexChanged(s);
        let on_apply = || SettingsMsg::CustomColorApply;
        let on_cancel = || SettingsMsg::CustomColorCancel;
        let on_history_select = |hex: String| SettingsMsg::CustomColorHistorySelect(hex);
        let body = crate::ui::components::color_picker::view(
            &state.fluent,
            t,
            ui,
            &state.settings.custom_color_history,
            on_hex,
            on_apply,
            on_cancel,
            on_history_select,
        );
        let card = container(body)
            .width(Length::Fixed(COLOR_POPOVER_WIDTH))
            .style(theme::style::subtle);
        let anchor = state.custom_color_anchor;
        crate::ui::components::popover::popover(
            card,
            move |bounds, viewport| {
                let mut x = anchor.x + COLOR_POPOVER_GAP;
                if x + bounds.width > viewport.width
                    && anchor.x - bounds.width - COLOR_POPOVER_GAP >= 0.0
                {
                    x = anchor.x - bounds.width - COLOR_POPOVER_GAP;
                }
                x = x.clamp(0.0, (viewport.width - bounds.width).max(0.0));
                let y = (anchor.y + COLOR_POPOVER_GAP)
                    .clamp(0.0, (viewport.height - bounds.height).max(0.0));
                Vector::new(x - bounds.x, y - bounds.y)
            },
            Some(Message::Settings(SettingsMsg::CustomColorCancel)),
        )
    } else {
        iced::widget::Space::new().into()
    };

    let stacked: iced::Element<'_, Message> = stack![
        base_layer,
        add_layer,
        about_layer,
        close_layer,
        details_layer,
        confirm_layer,
        update_layer,
        drop_overlay_layer,
        toast_layer,
        ctx_layer,
        shutdown_layer,
        speed_popover_layer,
        color_popover_layer,
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into();

    container(stacked)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[cfg(target_os = "windows")]
fn resize_placeholder_view(state: &Remotrix) -> Element<'static, Message> {
    use iced::widget::container;

    let base = container(iced::widget::Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::ui::theme::style::base_background);
    if state.window.maximized {
        #[allow(clippy::useless_conversion)]
        {
            iced::widget::opaque(base).into()
        }
    } else {
        stack![iced::widget::opaque(base), crate::ui::resize_frame::view(),]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
