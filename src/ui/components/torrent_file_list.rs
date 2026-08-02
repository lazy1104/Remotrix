use std::collections::HashSet;

use iced::widget::{button, column, container, row, rule, text};
use iced::{Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::ui::components::file_tree::{self, FileTreeNode};
use crate::ui::components::tooltip;
use crate::ui::components::tri_checkbox::{tri_checkbox, CheckState};
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

#[allow(clippy::too_many_arguments)]
pub fn view<'a, M>(
    fluent: &'a Fluent,
    _theme: &'a iced::Theme,
    title: String,
    subtitle: Option<String>,
    height: Length,
    nodes: &'a [FileTreeNode],
    expanded: &'a HashSet<String>,
    is_selected: &impl Fn(u64) -> bool,
    progress: Option<&impl Fn(u64) -> Option<(u64, u64)>>,
    enabled: bool,
    collapsed: bool,
    on_toggle: &'a impl Fn(String) -> M,
    on_expand: &impl Fn(String) -> M,
    on_select_all: M,
    on_select_none: M,
    on_toggle_collapse: Option<M>,
    scroll_offset: f32,
    on_scroll: &'a impl Fn(f32) -> M,
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
    if let Some(msg) = on_toggle_collapse {
        let (icon, label) = if collapsed {
            (icon::expand(), fluent.get(Tr::ExpandList))
        } else {
            (icon::collapse(), fluent.get(Tr::CollapseList))
        };
        let btn = button(icon.size(FONT_SMALL))
            .padding(PADDING_XS)
            .style(theme::style::button::toolbar_icon(false));
        let btn = if enabled {
            btn.on_press(msg)
        } else {
            btn.on_press_maybe(None)
        };
        header = header.push(tooltip::standard(
            btn,
            text(label).size(FONT_TINY),
            iced::widget::tooltip::Position::Bottom,
        ));
    }
    header = header
        .spacing(SPACE_SM)
        .align_y(Alignment::Center)
        .width(Length::Fill);

    let content = if collapsed {
        column![header].spacing(SPACE_MD).width(Length::Fill)
    } else {
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
                scroll_offset,
                on_scroll,
            ))
            .spacing(SPACE_MD)
            .width(Length::Fill)
    };

    container(content)
        .width(Length::Fill)
        .height(if collapsed { Length::Shrink } else { height })
        .padding(iced::Padding {
            top: SPACE_LG,
            right: SPACE_MD,
            bottom: if collapsed {
                SPACE_LG
            } else {
                PADDING_XS as f32
            },
            left: SPACE_MD,
        })
        .style(theme::style::tree_frame)
        .into()
}
