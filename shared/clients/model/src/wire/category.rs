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
mod tests {
    use super::*;

    #[test]
    fn overlay_queries() {
        let cat = ExtensionCategory::Overlay;
        assert!(cat.is_overlay());
        assert!(!cat.is_inline());
    }

    #[test]
    fn inline_queries() {
        let cat = ExtensionCategory::Inline;
        assert!(cat.is_inline());
        assert!(!cat.is_overlay());
    }

    #[test]
    fn copy_semantics() {
        let cat = ExtensionCategory::Overlay;
        let copied = cat;
        assert_eq!(cat, copied);
    }

    #[test]
    fn debug_format() {
        let cat = ExtensionCategory::Inline;
        let debug = format!("{cat:?}");
        assert!(debug.contains("Inline"));
    }

    #[test]
    fn serde_roundtrip_overlay() {
        let cat = ExtensionCategory::Overlay;
        let json = serde_json::to_string(&cat).unwrap();
        let deserialized: ExtensionCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(cat, deserialized);
    }

    #[test]
    fn serde_roundtrip_inline() {
        let cat = ExtensionCategory::Inline;
        let json = serde_json::to_string(&cat).unwrap();
        let deserialized: ExtensionCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(cat, deserialized);
    }

    #[test]
    fn eq_different_variants() {
        assert_ne!(ExtensionCategory::Overlay, ExtensionCategory::Inline);
    }
}
