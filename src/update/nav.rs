use iced::Task;

use crate::app::{pill_to_index, set_page, Remotrix};
use crate::message::{ConfirmAction, Message, NavMsg, Page};

pub(crate) fn handle(state: &mut Remotrix, msg: NavMsg) -> Task<Message> {
    match msg {
        NavMsg::NavigatePage(page) => {
            state.settings_ui.download_picker.close_history();
            if page == Page::Tasks && state.page == Page::Settings && state.settings_dirty {
                state.confirm = Some(ConfirmAction::LeaveSettings { target: page });
                state.confirm_anim.open();
            } else {
                set_page(state, page);
            }
            Task::none()
        }
        NavMsg::SetTaskFilter(filter) => {
            state.task_filter = filter;
            pill_to_index(state, crate::ui::category_bar::task_filter_index(filter));
            Task::none()
        }
        NavMsg::SetSettingsCategory(cat) => {
            state.settings_ui.download_picker.close_history();
            state.settings_cat = cat;
            pill_to_index(state, crate::ui::category_bar::settings_cat_index(cat));
            iced::widget::operation::scroll_to::<Message>(
                iced::widget::Id::new(crate::ui::settings_page::SETTINGS_SCROLL_ID),
                iced::widget::operation::AbsoluteOffset::<f32>::default(),
            )
        }
        NavMsg::SelectDetailsTab(tab) => {
            state.details.active_tab = tab;
            Task::none()
        }
    }
}
