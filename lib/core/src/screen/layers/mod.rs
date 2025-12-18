//! Layer implementations for compositor-based rendering
//!
//! Each layer represents a renderable component at a specific z-order.

mod base;
mod completion;
mod editor;
mod explorer;
mod leap;
mod settings_menu;
mod telescope;
mod which_key;

pub use {
    base::BaseLayer, completion::CompletionLayer, editor::EditorLayer, explorer::ExplorerLayer,
    leap::LeapLayer, settings_menu::SettingsMenuLayer, telescope::TelescopeLayer,
    which_key::WhichKeyLayer,
};
