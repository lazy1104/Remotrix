use std::collections::HashSet;

use iced::widget::{column, container, row, rule, text};
use iced::{Alignment, Element, Length};

use crate::i18n::Fluent;
use crate::ui::components::file_tree::{self, FileTreeNode};
use crate::ui::components::tri_checkbox::{tri_checkbox, CheckState};
use crate::ui::dims::*;
use crate::ui::theme;

#[allow(clippy::too_many_arguments)]
pub fn view<'a, M>(
    _fluent: &'a Fluent,
    _theme: &'a iced::Theme,
    title: String,
    subtitle: Option<String>,
    height: Length,
    nodes: &'a [FileTreeNode],
    expanded: &'a HashSet<String>,
    is_selected: &impl Fn(u64) -> bool,
    progress: Option<&impl Fn(u64) -> Option<(u64, u64)>>,
    enabled: bool,
    on_toggle: &'a impl Fn(String) -> M,
    on_expand: &impl Fn(String) -> M,
    on_select_all: M,
    on_select_none: M,
) -> Element<'a, M>
where
    M: Clone + 'a,
{
    let all_indices: Vec<u64> = nodes
        .iter()
        .flat_map(file_tree::descendant_indices)
        .collect();
    let selected_count = all_indices.iter().filter(|&&i| is_selected(i)).count();
    let total_count = all_indices.len();

    let master_state = if total_count > 0 && selected_count == total_count {
        CheckState::Checked
    } else if selected_count > 0 {
        CheckState::Partial
    } else {
        CheckState::Unchecked
    };

    let master_msg = if selected_count == total_count {
        on_select_none
    } else {
        on_select_all
    };

    let mut master = tri_checkbox(master_state).size(16.0);
    if enabled {
        master = master.on_toggle(move || master_msg.clone());
    } else {
        master = master.on_toggle_maybe(None::<fn() -> M>);
    }

    let mut header = row![
        master,
        text(title).size(FONT_MEDIUM),
        iced::widget::Space::new().width(Length::Fill),
    ];
    if let Some(sub) = subtitle {
        header = header.push(
            text(sub)
                .size(FONT_SMALL)
                .style(theme::style::text::secondary),
        );
    }
    header = header
        .spacing(SPACE_SM)
        .align_y(Alignment::Center)
        .width(Length::Fill);

    container(
        column![]
            .push(header)
            .push(rule::horizontal(1))
            .push(file_tree::view(
                nodes,
                expanded,
                is_selected,
                progress,
                enabled,
                on_toggle,
                on_expand,
            ))
            .spacing(SPACE_MD)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .height(height)
    .padding(iced::Padding {
        top: SPACE_LG,
        right: SPACE_MD,
        bottom: PADDING_XS as f32,
        left: SPACE_MD,
    })
    .style(theme::style::tree_frame)
    .into()
}
