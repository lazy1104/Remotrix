//! All `iced` view code for the Remotrix app.
//!
//! Each submodule owns a single self-contained piece of UI — a page, a
//! dialog, or a reusable component. `app.rs` composes them; the
//! `components` submodule bundles small widgets reused across multiple
//! pages.

pub mod about_dialog;
pub mod add_dialog;
pub mod animation;
pub mod category_bar;
pub mod close_dialog;
pub mod components;
pub mod confirm_dialog;
pub mod details_dialog;
pub mod dims;
pub mod hct;
pub mod icon;
pub mod icons;
pub mod resize_frame;
pub mod settings_page;
pub mod shutdown_card;
pub mod sidebar;
pub mod sort;
pub mod task_list;
pub mod theme;
pub mod title_bar;
pub mod update_dialog;
