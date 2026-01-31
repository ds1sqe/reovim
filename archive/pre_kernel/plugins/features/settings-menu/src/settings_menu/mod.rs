//! TUI Settings Menu
//!
//! An interactive menu for configuring editor settings with:
//! - Toggle options for booleans (checkbox style)
//! - Selection/cycle options for enums (dropdown style)
//! - Number input for numeric values
//! - Organized sections
//! - Vim-style navigation (j/k/h/l)
//! - Live preview of changes

pub mod item;
pub mod render;
pub mod state;

pub use {
    item::{ActionType, FlatItem, SettingItem, SettingSection, SettingValue},
    state::{
        MenuLayout, MessageKind, RegisteredOption, SectionMeta, SettingChange, SettingsInputMode,
        SettingsMenuState,
    },
};
