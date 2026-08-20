//! Reusable UI widgets shared across multiple pages: dialogs, popovers,
//! editors, progress indicators, etc. Each module owns a single
//! self-contained widget so they can be swapped or refactored
//! independently.

pub mod copyable_text;
pub mod ctx_input;
pub mod ctx_menu;
pub mod dialog;
pub mod drop_down;
pub mod drop_overlay;
pub mod expand;
pub mod file_tree;
pub mod key_value_list;
pub mod logo;
pub mod number_stepper;
pub mod path_picker;
pub mod piece_map;
pub mod secret_input;
pub mod slim_scrollable;
pub mod speed_hud;
pub mod spinner;
pub mod tag_picker;
pub mod toast;
pub mod tooltip;
pub mod torrent_file_list;
pub mod torrent_upload;
pub mod translate;
pub mod tri_checkbox;
pub mod truncated_text;

/// Standard control height used by inputs, pickers and steppers so they
/// line up vertically inside a settings row.
pub const CONTROL_HEIGHT: f32 = 33.0;
