use iced::Task;

use crate::app::{pill_settle_to_index, pill_to_index, Remotrix};
use crate::message::{ConfirmAction, Message, NavMsg, Page, SettingsCategory, TaskFilter};

#[derive(Debug, Clone, Copy)]
pub(crate) enum SwapTarget {
    Page(Page),
    Filter(TaskFilter),
    Category(SettingsCategory),
}

fn target_index(t: SwapTarget) -> usize {
    match t {
        SwapTarget::Page(p) => p.index(),
        SwapTarget::Filter(f) => f.index(),
        SwapTarget::Category(c) => c.index(),
    }
}

fn swap_index_delta(from: SwapTarget, to: SwapTarget) -> i8 {
    let a = target_index(from) as i32;
    let b = target_index(to) as i32;
    (a - b).clamp(-1, 1) as i8
}

fn page_index_delta(from: Page, to: Page) -> i8 {
    ((from.index() as i32) - (to.index() as i32)).clamp(-1, 1) as i8
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
                request_page_swap(state, page);
                // pill deferred to on_swap_anim commit so it moves with the page swap
            }
            Task::none()
        }
        NavMsg::SetTaskFilter(filter) => {
            request_swap(state, SwapTarget::Filter(filter));
            pill_to_index(state, filter.index());
            Task::none()
        }
        NavMsg::SetSettingsCategory(cat) => {
            state.settings_ui.download_picker.close_history();
            state.custom_color_picker_open = false;
            request_swap(state, SwapTarget::Category(cat));
            pill_to_index(state, cat.index());
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
    let from = match target {
        SwapTarget::Page(_) => SwapTarget::Page(state.page),
        SwapTarget::Filter(_) => SwapTarget::Filter(state.task_filter),
        SwapTarget::Category(_) => SwapTarget::Category(state.settings_cat),
    };
    state.swap_index_delta = swap_index_delta(from, target);
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
        SwapTarget::Page(p) => {
            state.page = p;
            let pill_index = match p {
                Page::Tasks => state.task_filter.index(),
                Page::Settings => state.settings_cat.index(),
            };
            pill_settle_to_index(state, pill_index);
        }
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
        let scroll_task = iced::widget::operation::scroll_to::<Message>(
            iced::widget::Id::new(crate::ui::settings_page::SETTINGS_SCROLL_ID),
            iced::widget::operation::AbsoluteOffset::<f32>::default(),
        );
        reset_swap_dir_when_done(state);
        return scroll_task;
    }
    reset_swap_dir_when_done(state);
    Task::none()
}

fn reset_swap_dir_when_done(state: &mut Remotrix) {
    if !state.swap.is_animating() && *state.swap.value() >= 1.0 - 0.001 {
        state.swap_index_delta = 0;
    }
}

fn request_page_swap(state: &mut Remotrix, target: Page) {
    if state.page == target && state.page_swap_pending.is_none() {
        return;
    }
    state.page_swap_index_delta = page_index_delta(state.page, target);
    state.page_swap_pending = Some(target);
    state.page_swap.set_target(crate::ui::animation::SWAP_MIN);
}

pub(crate) fn on_page_swap_anim(state: &mut Remotrix, value: f32) -> Task<Message> {
    if state.page_swap_pending.is_none()
        || state.page_swap.is_animating()
        || value > crate::ui::animation::SWAP_MIN + 0.01
    {
        return Task::none();
    }
    state.page_swap_pending = None;
    state.page_swap = crate::ui::animation::Animated::transition(
        crate::ui::animation::SWAP_MIN,
        crate::ui::animation::ease_out_cubic(crate::ui::animation::SWAP_ENTER_MS),
    );
    state.page_swap.set_target(1.0);
    if !state.page_swap.is_animating() && *state.page_swap.value() >= 1.0 - 0.001 {
        state.page_swap_index_delta = 0;
    }
    Task::none()
}
