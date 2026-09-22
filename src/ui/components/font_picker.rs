//! Searchable font-family dropdown used by Settings → Appearance → Font.
//!
//! The component owns its query string and open/closed state and renders
//! a trigger button plus an overlay (via [`crate::ui::components::drop_down::DropDown`])
//! holding a search field, a scrollable list, and an empty-state row.
//! The `on_select` callback fires when the user picks a row; selecting
//! a font closes the dropdown and clears the query.

use std::sync::Mutex;

use iced::widget::scrollable::Viewport;
use iced::widget::{button, column, container, row, text, text_input};
use iced::{Alignment, Element, Length};

use super::drop_down::{self, DropDown};
use super::slim_scrollable::slim_scrollable;
use crate::i18n::{Fluent, Locale, Tr};
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

#[derive(Debug, Clone)]
pub struct FontPickerUi {
    pub open: bool,
    pub query: String,
}

impl FontPickerUi {
    pub fn new() -> Self {
        Self {
            open: false,
            query: String::new(),
        }
    }
}

impl Default for FontPickerUi {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub enum FontPickerEntry {
    SystemDefault,
    Family(String),
}

impl FontPickerEntry {
    pub fn id(&self) -> &str {
        match self {
            FontPickerEntry::SystemDefault => "",
            FontPickerEntry::Family(id) => id.as_str(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FontPickerOption {
    pub entry: FontPickerEntry,
    pub display: String,
    pub search_key: String,
}

impl FontPickerOption {
    pub fn id(&self) -> &str {
        self.entry.id()
    }

    fn matches(&self, needle: &str) -> bool {
        let needle = needle.trim().to_lowercase();
        if needle.is_empty() {
            return true;
        }
        self.search_key.to_lowercase().contains(&needle)
    }
}

pub fn build_options(
    fluent: &Fluent,
    families: &[theme::FontFamily],
) -> &'static [FontPickerOption] {
    let mut cache = OPTIONS_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let needs_rebuild = cache
        .as_ref()
        .map(|(cached, _)| *cached != fluent.locale)
        .unwrap_or(true);
    if needs_rebuild {
        let mut out = Vec::with_capacity(families.len() + 1);
        let system_label = fluent.get(Tr::SystemDefault);
        out.push(FontPickerOption {
            entry: FontPickerEntry::SystemDefault,
            display: system_label.clone(),
            search_key: system_label,
        });
        for f in families {
            let display = if f.display == f.id {
                f.display.clone()
            } else {
                format!("{} ({})", f.display, f.id)
            };
            let search_key = if f.display == f.id {
                f.id.clone()
            } else {
                format!("{} {}", f.display, f.id)
            };
            out.push(FontPickerOption {
                entry: FontPickerEntry::Family(f.id.clone()),
                display,
                search_key,
            });
        }
        let leaked: &'static [FontPickerOption] = Box::leak(out.into_boxed_slice());
        *cache = Some((fluent.locale, leaked));
    }
    cache.as_ref().map(|(_, v)| *v).unwrap_or(&[])
}

static OPTIONS_CACHE: Mutex<Option<(Locale, &'static [FontPickerOption])>> = Mutex::new(None);

fn find_selected<'a>(
    options: &'a [FontPickerOption],
    selected_id: &str,
) -> Option<&'a FontPickerOption> {
    options.iter().find(|o| o.id() == selected_id)
}

const PICKER_WIDTH: f32 = 320.0;
const PICKER_LIST_HEIGHT: f32 = 240.0;

#[allow(clippy::too_many_arguments)]
pub fn view<'a, Message>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    state: &'a FontPickerUi,
    options: &'a [FontPickerOption],
    selected_id: &str,
    on_toggle: Message,
    on_dismiss: Message,
    on_query: impl Fn(String) -> Message + 'a,
    on_select: impl Fn(String) -> Message + 'a,
    on_scroll: impl Fn(Viewport) -> Message + 'a,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let selected = find_selected(options, selected_id);
    let placeholder = fluent.get(Tr::FontPickerSearch);
    let no_results = fluent.get(Tr::FontPickerNoResults);
    let secondary = theme::text_secondary(theme);

    let selected_label = selected.map(|o| o.display.clone()).unwrap_or_else(|| {
        if selected_id.is_empty() {
            fluent.get(Tr::SystemDefault)
        } else {
            selected_id.to_string()
        }
    });

    let trigger_btn = button(
        row![
            text(selected_label)
                .size(FONT_MEDIUM)
                .width(Length::Fill)
                .style(theme::style::text::secondary),
            icon::chevron_down().size(FONT_MEDIUM).color(secondary),
        ]
        .align_y(Alignment::Center)
        .spacing(SPACE_XS)
        .width(Length::Fill),
    )
    .on_press(on_toggle.clone())
    .padding(theme::INPUT_PADDING)
    .width(Length::Fixed(PICKER_WIDTH))
    .style(theme::style::button::trigger());

    let needle = state.query.clone();
    let query_input = theme::input_layout(
        text_input(&placeholder, &state.query)
            .on_input(on_query)
            .width(Length::Fill)
            .style(theme::style::input::standard),
    );

    let mut filtered: Vec<&FontPickerOption> =
        options.iter().filter(|o| o.matches(&needle)).collect();

    if !filtered.is_empty() {
        filtered.sort_by(|a, b| {
            let a_kind = match a.entry {
                FontPickerEntry::SystemDefault => 0,
                FontPickerEntry::Family(_) => 1,
            };
            let b_kind = match b.entry {
                FontPickerEntry::SystemDefault => 0,
                FontPickerEntry::Family(_) => 1,
            };
            a_kind
                .cmp(&b_kind)
                .then_with(|| a.display.to_lowercase().cmp(&b.display.to_lowercase()))
        });
    }

    let list_content: Element<'a, Message> = if filtered.is_empty() {
        container(
            text(no_results)
                .size(FONT_MEDIUM)
                .style(theme::style::text::secondary),
        )
        .center_x(Length::Fill)
        .padding([SPACE_2XL as u16, SPACE_XL as u16])
        .width(Length::Fill)
        .into()
    } else {
        let mut items: Vec<Element<'a, Message>> = Vec::with_capacity(filtered.len());
        for opt in filtered {
            let id = opt.id().to_string();
            let msg = on_select(id);
            let label = text(opt.display.clone())
                .size(FONT_MEDIUM)
                .width(Length::Fill);
            let is_selected = opt.id() == selected_id;
            let btn = if is_selected {
                button(label)
                    .on_press(msg)
                    .width(Length::Fill)
                    .padding(PADDING_BUTTON_XS)
                    .style(theme::style::button::chip())
            } else {
                button(label)
                    .on_press(msg)
                    .width(Length::Fill)
                    .padding(PADDING_BUTTON_XS)
                    .style(theme::style::button::text())
            };
            items.push(btn.into());
        }
        column(items).spacing(SPACE_XS).width(Length::Fill).into()
    };

    let body: Element<'a, Message> = container(
        column![
            query_input,
            slim_scrollable(
                list_content,
                iced::widget::Id::new("font-picker-list"),
                on_scroll,
            )
            .height(Length::Fixed(PICKER_LIST_HEIGHT)),
        ]
        .spacing(SPACE_SM)
        .width(Length::Fill),
    )
    .padding(PADDING_DROPDOWN)
    .style(theme::style::card)
    .into();

    DropDown::new(trigger_btn, body, state.open)
        .alignment(drop_down::Alignment::Bottom)
        .offset(drop_down::Offset::from(0.0))
        .on_dismiss(on_dismiss.clone())
        .width(Length::Fixed(PICKER_WIDTH))
        .into()
}
