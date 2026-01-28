//! User configuration for keybindings.
//!
//! Provides configuration loading from `~/.config/reovim/keymap.toml`.
//!
//! # Architecture
//!
//! This is separate from module configuration (`module/config.rs`) because:
//! - Module config controls WHAT modules load
//! - Keymap config controls HOW keys behave (user overrides)
//!
//! Both load from `~/.config/reovim/` but different files.

mod keymap;

pub use keymap::{ApplyStats, KeymapConfig, KeymapConfigError, ModeBindings, RemoveBindings};
