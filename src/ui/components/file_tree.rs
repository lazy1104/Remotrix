use std::collections::{HashMap, HashSet};

use iced::widget::scrollable::{self, Scrollbar};
use iced::widget::{button, container, keyed_column, row, text, Space};
use iced::{Alignment, Element, Length};

use crate::task::format_size;
use crate::ui::components::tri_checkbox::{tri_checkbox, CheckState};
use crate::ui::components::truncated_text::truncated_text;
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

const INDENT_STEP: f32 = 14.0;
const CHEVRON_SLOT: f32 = 20.0;
const NONE_COLOR: iced::Color = iced::Color::from_rgb(0.55, 0.55, 0.55);
const MAX_TREE_DEPTH: usize = 128;
const ROW_PITCH: f32 = 24.0;
const VIRTUAL_BUFFER_ROWS: usize = 40;
const VIRTUAL_WINDOW_ROWS: usize = 200;
const SPACER_TOP_KEY: u64 = u64::MAX - 1;
const SPACER_BOTTOM_KEY: u64 = u64::MAX;

#[derive(Debug, Clone)]
pub struct FileTreeNode {
    pub name: String,
    pub rel_path: String,
    pub is_dir: bool,
    pub file_index: Option<u64>,
    pub length: u64,
    pub children: Vec<FileTreeNode>,
}

pub fn build_tree(files: &[(u64, String, u64)]) -> Vec<FileTreeNode> {
    let mut roots: Vec<FileTreeNode> = Vec::new();
    for (idx, path, length) in files {
        let mut segments: Vec<String> = path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        if segments.is_empty() {
            roots.push(FileTreeNode {
                name: path.clone(),
                rel_path: path.clone(),
                is_dir: false,
                file_index: Some(*idx),
                length: *length,
                children: Vec::new(),
            });
            continue;
        }
        if segments.len() > MAX_TREE_DEPTH {
            let tail = segments.split_off(MAX_TREE_DEPTH - 1).join("/");
            segments.push(tail);
        }
        let mut children = &mut roots;
        let mut rel = String::new();
        for (i, seg) in segments.iter().enumerate() {
            if !rel.is_empty() {
                rel.push('/');
            }
            rel.push_str(seg);
            let is_last = i == segments.len() - 1;
            match children.iter().position(|n| n.name == *seg) {
                Some(p) => {
                    if is_last {
                        let node = &mut children[p];
                        if node.file_index.is_some() {
                            let parent_rel = rel
                                .strip_suffix(seg)
                                .map(|s| s.trim_end_matches('/').to_string())
                                .unwrap_or_default();
                            let mut n = 2;
                            loop {
                                let candidate = format!("{seg} (~{n})");
                                if !children.iter().any(|c| c.name == candidate) {
                                    let cand_rel = if parent_rel.is_empty() {
                                        candidate.clone()
                                    } else {
                                        format!("{parent_rel}/{candidate}")
                                    };
                                    children.push(FileTreeNode {
                                        name: candidate,
                                        rel_path: cand_rel,
                                        is_dir: false,
                                        file_index: Some(*idx),
                                        length: *length,
                                        children: Vec::new(),
                                    });
                                    break;
                                }
                                n += 1;
                            }
                        } else {
                            node.is_dir = false;
                            node.file_index = Some(*idx);
                            node.length = *length;
                        }
                        break;
                    }
                    let node = &mut children[p];
                    children = &mut node.children;
                }
                None => {
                    children.push(FileTreeNode {
                        name: seg.clone(),
                        rel_path: rel.clone(),
                        is_dir: !is_last,
                        file_index: if is_last { Some(*idx) } else { None },
                        length: if is_last { *length } else { 0 },
                        children: Vec::new(),
                    });
                    if !is_last {
                        let last = children.len() - 1;
                        let node = &mut children[last];
                        children = &mut node.children;
                    }
                }
            }
        }
    }
    for root in roots.iter_mut() {
        sum_lengths(root);
    }
    roots
}

/// Flip the `selected` flag of every entry whose index is in `indices`. If that
/// would leave nothing selected among `entries`, the flip is reverted.
pub fn flip_with_guard(entries: &mut [(u64, bool)], indices: &[u64]) {
    let mut flipped: Vec<u64> = Vec::new();
    for &i in indices {
        if let Some(entry) = entries.iter_mut().find(|(idx, _)| *idx == i) {
            entry.1 = !entry.1;
            if !entry.1 {
                flipped.push(i);
            }
        }
    }
    if entries.iter().all(|(_, selected)| !selected) {
        for i in flipped {
            if let Some(entry) = entries.iter_mut().find(|(idx, _)| *idx == i) {
                entry.1 = true;
            }
        }
    }
}

fn sum_lengths(node: &mut FileTreeNode) -> u64 {
    if node.is_dir {
        let total: u64 = node.children.iter_mut().map(sum_lengths).sum();
        node.length = total;
        total
    } else {
        node.length
    }
}

fn collect_indices(node: &FileTreeNode, out: &mut Vec<u64>) {
    if let Some(i) = node.file_index {
        out.push(i);
    }
    for child in &node.children {
        collect_indices(child, out);
    }
}

pub fn descendant_indices(node: &FileTreeNode) -> Vec<u64> {
    let mut out = Vec::new();
    collect_indices(node, &mut out);
    out
}

pub fn find_node<'a>(nodes: &'a [FileTreeNode], path: &str) -> Option<&'a FileTreeNode> {
    for node in nodes {
        if node.rel_path == path {
            return Some(node);
        }
        if let Some(found) = find_node(&node.children, path) {
            return Some(found);
        }
    }
    None
}

pub fn collect_dir_paths(nodes: &[FileTreeNode], out: &mut HashSet<String>) {
    for node in nodes {
        if node.is_dir {
            out.insert(node.rel_path.clone());
            collect_dir_paths(&node.children, out);
        }
    }
}

#[derive(Clone, Default)]
struct DirAgg {
    selected: u32,
    total: u32,
    done: u64,
}

fn collect_aggregates(
    node: &FileTreeNode,
    is_selected: &impl Fn(u64) -> bool,
    progress: Option<&impl Fn(u64) -> Option<(u64, u64)>>,
    out: &mut HashMap<String, DirAgg>,
) -> DirAgg {
    let mut agg = DirAgg::default();
    for child in &node.children {
        let child_agg = collect_aggregates(child, is_selected, progress, out);
        agg.selected += child_agg.selected;
        agg.total += child_agg.total;
        agg.done += child_agg.done;
    }
    if node.is_dir {
        out.insert(node.rel_path.clone(), agg.clone());
    } else {
        let idx = node.file_index.unwrap_or(0);
        agg.selected += u32::from(is_selected(idx));
        agg.total += 1;
        if let Some(p) = progress {
            agg.done += p(idx).map(|(d, _)| d).unwrap_or(0);
        }
    }
    agg
}

fn flatten_visible<'a>(
    node: &'a FileTreeNode,
    expanded: &HashSet<String>,
    out: &mut Vec<(&'a FileTreeNode, u32)>,
    depth: u32,
) {
    out.push((node, depth));
    if node.is_dir && expanded.contains(&node.rel_path) {
        for child in &node.children {
            flatten_visible(child, expanded, out, depth + 1);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn view<'a, M>(
    nodes: &'a [FileTreeNode],
    expanded: &'a HashSet<String>,
    is_selected: &impl Fn(u64) -> bool,
    progress: Option<&impl Fn(u64) -> Option<(u64, u64)>>,
    enabled: bool,
    on_toggle: &'a impl Fn(String) -> M,
    on_expand: &impl Fn(String) -> M,
    scroll_offset: f32,
    on_scroll: &'a impl Fn(f32) -> M,
) -> Element<'a, M>
where
    M: Clone + 'a,
{
    let mut aggs: HashMap<String, DirAgg> = HashMap::new();
    for node in nodes {
        collect_aggregates(node, is_selected, progress, &mut aggs);
    }

    let mut rows: Vec<(&FileTreeNode, u32)> = Vec::with_capacity(nodes.len());
    for node in nodes {
        flatten_visible(node, expanded, &mut rows, 0);
    }
    let total = rows.len();

    let first = (scroll_offset / ROW_PITCH).floor().max(0.0) as usize;
    let first = first.saturating_sub(VIRTUAL_BUFFER_ROWS).min(total);
    let last = (first + VIRTUAL_WINDOW_ROWS).min(total);

    let default_agg = DirAgg::default();
    let mut items: Vec<(u64, Element<'a, M>)> = Vec::with_capacity(last - first + 2);
    items.push((
        SPACER_TOP_KEY,
        Space::new()
            .height(Length::Fixed(first as f32 * ROW_PITCH))
            .into(),
    ));
    for (i, (node, depth)) in rows[first..last].iter().enumerate() {
        let el = if node.is_dir {
            let agg = aggs.get(&node.rel_path).unwrap_or(&default_agg);
            render_dir_row(
                node, *depth, expanded, agg, progress, enabled, on_toggle, on_expand,
            )
        } else {
            render_file_row(node, *depth, is_selected, progress, enabled, on_toggle)
        };
        items.push(((first + i) as u64, el));
    }
    items.push((
        SPACER_BOTTOM_KEY,
        Space::new()
            .height(Length::Fixed((total - last) as f32 * ROW_PITCH))
            .into(),
    ));
    let col = keyed_column(items).spacing(SPACE_NONE).width(Length::Fill);

    iced::widget::scrollable(
        container(col)
            .width(Length::Fill)
            .padding(iced::padding::bottom(5.0)),
    )
    .direction(scrollable::Direction::Vertical(
        Scrollbar::new().width(6.0).scroller_width(6.0),
    ))
    .spacing(SPACE_SCROLL)
    .style(theme::style::scrollable::standard)
    .height(Length::Fill)
    .on_scroll(move |v: scrollable::Viewport| on_scroll(v.absolute_offset().y))
    .into()
}

#[allow(clippy::too_many_arguments)]
fn render_dir_row<'a, M>(
    node: &'a FileTreeNode,
    depth: u32,
    expanded: &'a HashSet<String>,
    agg: &DirAgg,
    progress: Option<&impl Fn(u64) -> Option<(u64, u64)>>,
    enabled: bool,
    on_toggle: &'a impl Fn(String) -> M,
    on_expand: &impl Fn(String) -> M,
) -> Element<'a, M>
where
    M: Clone + 'a,
{
    let expanded_flag = expanded.contains(&node.rel_path);
    let chevron = if expanded_flag {
        button(icon::chevron_down().size(FONT_SMALL))
            .on_press(on_expand(node.rel_path.clone()))
            .padding(PADDING_XS)
            .style(theme::style::button::toolbar_icon(false))
    } else {
        button(icon::chevron_right().size(FONT_SMALL))
            .on_press(on_expand(node.rel_path.clone()))
            .padding(PADDING_XS)
            .style(theme::style::button::toolbar_icon(false))
    };

    let check_state = if agg.total > 0 && agg.selected == agg.total {
        CheckState::Checked
    } else if agg.selected > 0 {
        CheckState::Partial
    } else {
        CheckState::Unchecked
    };
    let mut chk = tri_checkbox(check_state).size(16.0);
    if enabled {
        let rel = node.rel_path.clone();
        chk = chk.on_toggle_maybe(Some(move || on_toggle(rel.clone())));
    } else {
        chk = chk.on_toggle_maybe(None::<fn() -> M>);
    }

    let size_text = if progress.is_some() {
        format!("{} / {}", format_size(agg.done), format_size(node.length))
    } else {
        format_size(node.length)
    };

    row![]
        .push(Space::new().width(Length::Fixed(depth as f32 * INDENT_STEP)))
        .push(chevron)
        .push(chk)
        .push(icon::folder().size(FONT_ICON).color(NONE_COLOR))
        .push(
            truncated_text(node.name.clone())
                .size(FONT_MEDIUM)
                .max_lines(1)
                .width(Length::Fill),
        )
        .push(
            text(size_text)
                .size(FONT_SMALL)
                .style(theme::style::text::secondary),
        )
        .spacing(SPACE_SM)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .height(Length::Fixed(ROW_PITCH))
        .into()
}

fn render_file_row<'a, M>(
    node: &'a FileTreeNode,
    depth: u32,
    is_selected: &impl Fn(u64) -> bool,
    progress: Option<&impl Fn(u64) -> Option<(u64, u64)>>,
    enabled: bool,
    on_toggle: &'a impl Fn(String) -> M,
) -> Element<'a, M>
where
    M: Clone + 'a,
{
    let idx = node.file_index.unwrap_or(0);
    let mut chk = tri_checkbox(if is_selected(idx) {
        CheckState::Checked
    } else {
        CheckState::Unchecked
    })
    .size(16.0);
    if enabled {
        let rel = node.rel_path.clone();
        chk = chk.on_toggle_maybe(Some(move || on_toggle(rel.clone())));
    } else {
        chk = chk.on_toggle_maybe(None::<fn() -> M>);
    }

    let size_text = if let Some(p) = progress {
        let done = p(idx).map(|(d, _)| d).unwrap_or(0);
        format!("{} / {}", format_size(done), format_size(node.length))
    } else {
        format_size(node.length)
    };

    row![]
        .push(Space::new().width(Length::Fixed(depth as f32 * INDENT_STEP)))
        .push(Space::new().width(Length::Fixed(CHEVRON_SLOT)))
        .push(chk)
        .push(icon::file().size(FONT_ICON).color(NONE_COLOR))
        .push(
            truncated_text(node.name.clone())
                .size(FONT_MEDIUM)
                .max_lines(1)
                .width(Length::Fill),
        )
        .push(
            text(size_text)
                .size(FONT_SMALL)
                .style(theme::style::text::secondary),
        )
        .spacing(SPACE_SM)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .height(Length::Fixed(ROW_PITCH))
        .into()
}
