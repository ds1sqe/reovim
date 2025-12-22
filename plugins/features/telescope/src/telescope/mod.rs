//! Telescope - Fuzzy finder module
//!
//! This module provides a Telescope-like fuzzy finder system with:
//! - Trait-based picker system for extensibility
//! - Nucleo-powered fuzzy matching
//! - Preview panel support
//! - Multiple built-in pickers (files, buffers, grep, etc.)

mod events;
pub mod item;
pub mod matcher;
pub mod picker;
pub mod state;

pub use {
    events::*,
    item::{TelescopeData, TelescopeItem},
    matcher::TelescopeMatcher,
    picker::{Picker, PickerContext, TelescopeAction},
    state::{PreviewContent, TelescopeLayout, TelescopeState},
};
