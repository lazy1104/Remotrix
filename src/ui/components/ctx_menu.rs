use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use iced::widget::{button, column, container, text, text_input};
use iced::{Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{AddField, CtxTarget, Message};
use crate::ui::dims::*;
use crate::ui::theme;

#[derive(Debug, Clone, Copy, Default)]
pub struct CtxCursor {
    pub selection: Option<(usize, usize)>,
    pub caret: usize,
    pub pending_caret: Option<(usize, usize)>,
}

pub type CtxMirrors = HashMap<CtxTarget, Rc<RefCell<CtxCursor>>>;

pub fn is_secure_target(target: CtxTarget) -> bool {
    matches!(
        target,
        CtxTarget::AddAdvanced(AddField::HttpPasswd | AddField::ProxyPassword)
            | CtxTarget::DetailsAdvanced(AddField::HttpPasswd | AddField::ProxyPassword)
    )
}

pub fn merge_paste(old: &str, cur: &CtxCursor, pasted: &str) -> (String, usize) {
    let v = text_input::Value::new(old);
    let (s, e) = cur
        .selection
        .map_or((cur.caret, cur.caret), |(s, e)| (s, e));
    (
        format!("{}{}{}", v.until(s), pasted, v.select(e, v.len())),
        s + pasted.len(),
    )
}

pub fn menu<'a>(
    fluent: &'a Fluent,
    selected: Option<String>,
    clipboard: Option<String>,
    target: CtxTarget,
) -> Element<'a, Message> {
    let copy_text = selected.filter(|s| !s.is_empty() && !is_secure_target(target));
    let paste_text = clipboard.filter(|t| !t.is_empty());
    container(
        column![
            button(text(fluent.get(Tr::Copy)).size(FONT_MEDIUM))
                .on_press_maybe(copy_text.map(Message::CtxCopy))
                .padding(PADDING_DROPDOWN)
                .width(Length::Fill)
                .style(theme::style::button::picker_item()),
            button(text(fluent.get(Tr::Paste)).size(FONT_MEDIUM))
                .on_press_maybe(paste_text.map(|t| Message::CtxPaste(target, t)))
                .padding(PADDING_DROPDOWN)
                .width(Length::Fill)
                .style(theme::style::button::picker_item()),
        ]
        .spacing(SPACE_XS)
        .width(Length::Fill),
    )
    .padding(PADDING_DROPDOWN)
    .width(Length::Fixed(140.0))
    .style(theme::style::card)
    .into()
}
