//! Telescope - Fuzzy finder module
//!
//! This module provides a Telescope-like fuzzy finder system with:
//! - Trait-based picker system for extensibility
//! - Nucleo-powered fuzzy matching
//! - Preview panel support
//! - Multiple built-in pickers (files, buffers, grep, etc.)

pub mod item;
pub mod matcher;
pub mod picker;
pub mod state;

pub use item::{TelescopeData, TelescopeItem};
pub use matcher::TelescopeMatcher;
pub use picker::{Picker, PickerContext, TelescopeAction};
pub use state::{PreviewContent, TelescopeLayout, TelescopeState};
