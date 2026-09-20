use iced::Task;

use crate::app::{pill_to_index, Remotrix};
use crate::message::{ConfirmAction, Message, NavMsg, Page};

pub(crate) fn handle(state: &mut Remotrix, msg: NavMsg) -> Task<Message> {
    match msg {
        NavMsg::NavigatePage(page) => {
            state.settings_ui.download_picker.close_history();
            if page == Page::Tasks && state.page == Page::Settings && state.settings_dirty {
                state.confirm = Some(ConfirmAction::LeaveSettings { target: page });
                state.confirm_anim.open();
                return Task::none();
            }
            state.page = page;
            let pill_index = match page {
                Page::Tasks => state.task_filter.index(),
                Page::Settings => state.settings_cat.index(),
            };
            pill_to_index(state, pill_index);
            if matches!(page, Page::Settings) {
                return iced::widget::operation::scroll_to::<Message>(
                    iced::widget::Id::new(crate::ui::settings_page::SETTINGS_SCROLL_ID),
                    iced::widget::operation::AbsoluteOffset::<f32>::default(),
                );
            }
            Task::none()
        }
        NavMsg::SetTaskFilter(filter) => {
            state.task_filter = filter;
            pill_to_index(state, filter.index());
            Task::none()
        }
        NavMsg::SetSettingsCategory(cat) => {
            state.settings_ui.download_picker.close_history();
            state.custom_color_picker_open = false;
            state.settings_cat = cat;
            pill_to_index(state, cat.index());
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
