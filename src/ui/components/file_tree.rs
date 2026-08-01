use std::collections::HashSet;

use iced::widget::{button, checkbox, column, container, progress_bar, row, text, Space};
use iced::{Alignment, Element, Length};

use crate::task::format_size;
use crate::ui::components::truncated_text::truncated_text;
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

const INDENT_STEP: f32 = 14.0;
const CHEVRON_SLOT: f32 = 20.0;
const NONE_COLOR: iced::Color = iced::Color::from_rgb(0.55, 0.55, 0.55);
const MAX_TREE_DEPTH: usize = 128;

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

fn dir_state(node: &FileTreeNode, is_selected: &impl Fn(u64) -> bool) -> Option<bool> {
    let indices = descendant_indices(node);
    let mut all = true;
    let mut any = false;
    for i in indices {
        let selected = is_selected(i);
        all &= selected;
        any |= selected;
    }
    if all {
        Some(true)
    } else if any {
        Some(false)
    } else {
        None
    }
}

pub fn view<'a, M>(
    nodes: &'a [FileTreeNode],
    expanded: &'a HashSet<String>,
    is_selected: &impl Fn(u64) -> bool,
    progress: Option<&impl Fn(u64) -> Option<(u64, u64)>>,
    enabled: bool,
    on_toggle: &'a impl Fn(String) -> M,
    on_expand: &impl Fn(String) -> M,
) -> Element<'a, M>
where
    M: Clone + 'a,
{
    let mut col = column![].spacing(SPACE_SM).width(Length::Fill);
    for node in nodes {
        col = col.push(render_node(
            node,
            0,
            expanded,
            is_selected,
            progress,
            enabled,
            on_toggle,
            on_expand,
        ));
    }
    col.into()
}

#[allow(clippy::too_many_arguments)]
fn render_node<'a, M>(
    node: &'a FileTreeNode,
    depth: u32,
    expanded: &'a HashSet<String>,
    is_selected: &impl Fn(u64) -> bool,
    progress: Option<&impl Fn(u64) -> Option<(u64, u64)>>,
    enabled: bool,
    on_toggle: &'a impl Fn(String) -> M,
    on_expand: &impl Fn(String) -> M,
) -> Element<'a, M>
where
    M: Clone + 'a,
{
    if node.is_dir {
        let mut col = column![].spacing(SPACE_XS).width(Length::Fill);
        col = col.push(render_dir_row(
            node,
            depth,
            expanded,
            is_selected,
            enabled,
            on_toggle,
            on_expand,
        ));
        if expanded.contains(&node.rel_path) {
            for child in &node.children {
                col = col.push(render_node(
                    child,
                    depth + 1,
                    expanded,
                    is_selected,
                    progress,
                    enabled,
                    on_toggle,
                    on_expand,
                ));
            }
        }
        col.into()
    } else {
        render_file_row(node, depth, is_selected, progress, enabled, on_toggle)
    }
}

fn render_dir_row<'a, M>(
    node: &'a FileTreeNode,
    depth: u32,
    expanded: &'a HashSet<String>,
    is_selected: &impl Fn(u64) -> bool,
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

    let state = dir_state(node, is_selected);
    let on_toggle_maybe = if enabled {
        Some(on_toggle(node.rel_path.clone()))
    } else {
        None
    };
    let tri_btn = match state {
        Some(true) => button(icon::circle_check().size(FONT_MEDIUM))
            .on_press_maybe(on_toggle_maybe)
            .padding(PADDING_XS)
            .style(theme::style::button::toolbar_icon(true)),
        Some(false) => button(icon::minus().size(FONT_MEDIUM))
            .on_press_maybe(on_toggle_maybe)
            .padding(PADDING_XS)
            .style(theme::style::button::toolbar_icon(true)),
        None => button(icon::square().size(FONT_MEDIUM).color(NONE_COLOR))
            .on_press_maybe(on_toggle_maybe)
            .padding(PADDING_XS)
            .style(theme::style::button::toolbar_icon(false)),
    };

    row![]
        .push(Space::new().width(Length::Fixed(depth as f32 * INDENT_STEP)))
        .push(chevron)
        .push(tri_btn)
        .push(icon::folder().size(FONT_ICON).color(NONE_COLOR))
        .push(
            truncated_text(node.name.clone())
                .size(FONT_MEDIUM)
                .max_lines(1)
                .width(Length::Fill),
        )
        .push(
            text(format_size(node.length))
                .size(FONT_SMALL)
                .style(theme::style::text::secondary),
        )
        .spacing(SPACE_SM)
        .align_y(Alignment::Center)
        .width(Length::Fill)
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
    let mut chk = checkbox(is_selected(idx)).size(16.0);
    if enabled {
        let rel = node.rel_path.clone();
        chk = chk.on_toggle_maybe(Some(move |_| on_toggle(rel.clone())));
    } else {
        chk = chk.on_toggle_maybe(None::<fn(bool) -> M>);
    }

    let mut content = column![].spacing(SPACE_XS).width(Length::Fill);
    content = content.push(
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
                text(format_size(node.length))
                    .size(FONT_SMALL)
                    .style(theme::style::text::secondary),
            )
            .spacing(SPACE_SM)
            .align_y(Alignment::Center)
            .width(Length::Fill),
    );

    if let Some(p) = progress {
        if let Some((done, total)) = p(idx) {
            let pct = if total == 0 {
                0.0
            } else {
                (done as f64 / total as f64 * 100.0).min(100.0) as f32
            };
            content = content.push(
                container(progress_bar(0.0..=100.0, pct).girth(Length::Fixed(4.0)))
                    .width(Length::Fill),
            );
        }
    }

    content.into()
}
