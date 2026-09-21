//! All `iced` view code for the Remotrix app.
//!
//! Each submodule owns a single self-contained piece of UI — a page, a
//! dialog, or a reusable component. `app.rs` composes them; the
//! `components` submodule bundles small widgets reused across multiple
//! pages.

pub mod about_dialog;
pub mod add_dialog;
pub mod animation;
pub mod border_bar;
pub mod category_bar;
pub mod close_dialog;
pub mod color;
pub mod components;
pub mod confirm_dialog;
pub mod details_dialog;
pub mod dims;
pub mod icon;
pub mod resize_frame;
pub mod scroll_anim;
pub mod settings;
pub mod sidebar;

pub use settings::*;

#[deprecated(note = "kept as facade for crate::ui::settings_page paths")]
pub mod settings_page {
    pub use super::settings::*;
}
pub mod sort;
pub mod task_list;
pub mod title_bar;
pub mod update_dialog;
pub mod view;

pub use color::hct;
pub use color::theme;
