//! Microscope - Fuzzy finder module
//!
//! This module provides a Microscope-like fuzzy finder system with:
//! - Trait-based picker system for extensibility
//! - Nucleo-powered streaming fuzzy matching
//! - Helix-style bottom-anchored layout with preview
//! - Modal editing (Normal/Insert modes) in prompt
//! - Multiple built-in pickers (files, buffers, grep, etc.)
//!
//! # Streaming Architecture
//!
//! The matcher uses nucleo's `Nucleo<T>` API for non-blocking fuzzy matching:
//! - Items are pushed via `Injector` from background tasks
//! - `tick()` processes matches without blocking input
//! - `snapshot()` retrieves current results instantly
//!
//! # Helix-Style Layout
//!
//! ```text
//! +---------------------------------------------------------------+
//! |                      Editor Content                            |
//! +------------------ y = screen_height - picker_height -----------+
//! | Results (40%)                    | Preview (60%)               |
//! +----------------------------------+------------------------------+
//! | status: 4/128 files                    <CR> Open | <Esc> Close |
//! +---------------------------------------------------------------+
//! ```

pub mod item;
pub mod layout;
pub mod matcher;
pub mod picker;
pub mod state;

pub use {
    item::{MicroscopeData, MicroscopeItem},
    layout::{LayoutBounds, LayoutConfig, PanelBounds, calculate_layout, visible_item_count},
    matcher::{MatcherItem, MatcherStatus, MicroscopeMatcher, push_item, push_items},
    picker::{BufferInfo, MicroscopeAction, Picker, PickerContext, PickerRegistry},
    state::{LoadingState, MicroscopeLayout, MicroscopeState, PreviewContent, PromptMode},
};
