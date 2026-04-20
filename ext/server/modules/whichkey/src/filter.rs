//! Which-key filter configuration (#459).
//!
//! [`WhichKeyFilterConfig`] controls which bindings appear in the
//! which-key popup. Stored as a [`SessionExtension`] in the client's
//! `ExtensionMap` and consumed by [`WhichKeyBridge::snapshot()`].
//!
//! # Filter Types
//!
//! - **Category**: Show only bindings from specific categories (e.g., "motion", "operator")
//! - **Key pattern**: Match continuation keys against a substring pattern
//! - **Layer**: Show only user-defined or only default bindings

use {
    reovim_driver_text_input::{BindingInfo, BindingLayer, KeySequence},
    reovim_driver_text_session::SessionExtension,
};

/// Filter configuration for which-key hints.
///
/// Stored as a [`SessionExtension`] in the client's `ExtensionMap`.
/// The [`WhichKeyBridge`] reads this to filter continuations before
/// producing JSON output.
///
/// [`WhichKeyBridge`]: crate::WhichKeyBridge
pub struct WhichKeyFilterConfig {
    /// Filter by categories. Only bindings whose category matches one of
    /// these strings will be shown. `None` means no category filtering.
    pub categories: Option<Vec<String>>,
    /// Filter by key display pattern. Only continuations whose key
    /// display string contains this substring are shown.
    /// `None` means no key pattern filtering.
    pub key_pattern: Option<String>,
    /// Filter by binding layer. `None` means no layer filtering.
    pub layer_filter: Option<BindingLayerFilter>,
}

/// Which binding layers to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingLayerFilter {
    /// Show only user-defined bindings (`BindingLayer::User`).
    UserOnly,
    /// Show only default bindings (`BindingLayer::Policy` + `BindingLayer::Base`).
    DefaultsOnly,
    /// Show bindings from exactly this layer.
    Specific(BindingLayer),
}

impl SessionExtension for WhichKeyFilterConfig {
    fn create() -> Self {
        Self {
            categories: None,
            key_pattern: None,
            layer_filter: None,
        }
    }
}

impl WhichKeyFilterConfig {
    /// Check if a binding passes all active filters.
    #[must_use]
    pub fn matches(&self, keys: &KeySequence, info: &BindingInfo) -> bool {
        // Category filter
        if let Some(cats) = &self.categories {
            match info.category {
                Some(cat) => {
                    if !cats.iter().any(|c| c == cat) {
                        return false;
                    }
                }
                None => return false,
            }
        }

        // Key pattern filter
        if let Some(pattern) = &self.key_pattern {
            let key_display = format!("{keys}");
            if !key_display.contains(pattern.as_str()) {
                return false;
            }
        }

        // Layer filter
        if let Some(layer_filter) = &self.layer_filter {
            match layer_filter {
                BindingLayerFilter::UserOnly => {
                    if info.layer != BindingLayer::User {
                        return false;
                    }
                }
                BindingLayerFilter::DefaultsOnly => {
                    if info.layer == BindingLayer::User {
                        return false;
                    }
                }
                BindingLayerFilter::Specific(layer) => {
                    if info.layer != *layer {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Clear all filters.
    pub fn clear(&mut self) {
        self.categories = None;
        self.key_pattern = None;
        self.layer_filter = None;
    }
}

#[cfg(test)]
#[path = "filter_tests.rs"]
mod tests;
