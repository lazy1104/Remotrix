use iced::Task;

use crate::app::{pill_to_index, Remotrix};
use crate::message::{ConfirmAction, Message, NavMsg, Page, SettingsCategory, TaskFilter};

#[derive(Debug, Clone, Copy)]
pub(crate) enum SwapTarget {
    Page(Page),
    Filter(TaskFilter),
    Category(SettingsCategory),
}

pub(crate) fn handle(state: &mut Remotrix, msg: NavMsg) -> Task<Message> {
    match msg {
        NavMsg::NavigatePage(page) => {
            state.settings_ui.download_picker.close_history();
            if page == Page::Tasks && state.page == Page::Settings && state.settings_dirty {
                state.confirm = Some(ConfirmAction::LeaveSettings { target: page });
                state.confirm_anim.open();
            } else {
                request_swap(state, SwapTarget::Page(page));
                let pill_index = match page {
                    Page::Tasks => crate::ui::category_bar::task_filter_index(state.task_filter),
                    Page::Settings => {
                        crate::ui::category_bar::settings_cat_index(state.settings_cat)
                    }
                };
                pill_to_index(state, pill_index);
            }
            Task::none()
        }
        NavMsg::SetTaskFilter(filter) => {
            request_swap(state, SwapTarget::Filter(filter));
            pill_to_index(state, crate::ui::category_bar::task_filter_index(filter));
            Task::none()
        }
        NavMsg::SetSettingsCategory(cat) => {
            state.settings_ui.download_picker.close_history();
            state.custom_color_picker_open = false;
            request_swap(state, SwapTarget::Category(cat));
            pill_to_index(state, crate::ui::category_bar::settings_cat_index(cat));
            Task::none()
        }
        NavMsg::SelectDetailsTab(tab) => {
            state.details.active_tab = tab;
            Task::none()
        }
    }
}

fn request_swap(state: &mut Remotrix, target: SwapTarget) {
    let already = match target {
        SwapTarget::Page(p) => state.page == p,
        SwapTarget::Filter(f) => state.task_filter == f,
        SwapTarget::Category(c) => state.settings_cat == c,
    };
    if already && state.swap_pending.is_none() {
        return;
    }
    state.swap_pending = Some(target);
    state.swap.set_target(crate::ui::animation::SWAP_MIN);
}

pub(crate) fn on_swap_anim(state: &mut Remotrix, value: f32) -> Task<Message> {
    if state.swap_pending.is_none()
        || state.swap.is_animating()
        || value > crate::ui::animation::SWAP_MIN + 0.01
    {
        return Task::none();
    }
    let target = state.swap_pending.take().unwrap();
    match target {
        SwapTarget::Page(p) => state.page = p,
        SwapTarget::Filter(f) => state.task_filter = f,
        SwapTarget::Category(c) => state.settings_cat = c,
    }
    state.swap = crate::ui::animation::Animated::transition(
        crate::ui::animation::SWAP_MIN,
        crate::ui::animation::ease_out_cubic(crate::ui::animation::SWAP_ENTER_MS),
    );
    state.swap.set_target(1.0);
    if matches!(
        target,
        SwapTarget::Page(Page::Settings) | SwapTarget::Category(_)
    ) {
        return iced::widget::operation::scroll_to::<Message>(
            iced::widget::Id::new(crate::ui::settings_page::SETTINGS_SCROLL_ID),
            iced::widget::operation::AbsoluteOffset::<f32>::default(),
        );
    }
    Task::none()
}
