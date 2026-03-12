//! Extension category classification.
//!
//! `ExtensionCategory` describes how an extension renders relative to
//! buffer content. This is informational metadata — the client uses it
//! to decide rendering strategy, not a directive from the server.

use serde::{Deserialize, Serialize};

/// How an extension renders relative to buffer content.
///
/// Clients use this to choose between overlay rendering (popups, floating
/// panels) and inline rendering (underlines, gutter signs, virtual text).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum ExtensionCategory {
    /// Floats above content (popups, menus, toasts).
    ///
    /// Examples: completion, cmdline, microscope, hover.
    Overlay,

    /// Modifies buffer rendering inline (underlines, gutter signs).
    ///
    /// Examples: diagnostics, range-finder labels.
    Inline,
}

impl ExtensionCategory {
    /// Check if this is an overlay extension.
    #[must_use]
    pub const fn is_overlay(self) -> bool {
        matches!(self, Self::Overlay)
    }

    /// Check if this is an inline extension.
    #[must_use]
    pub const fn is_inline(self) -> bool {
        matches!(self, Self::Inline)
    }
}

#[cfg(test)]
#[path = "category_tests.rs"]
mod tests;
