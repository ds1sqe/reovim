//! Domain-neutral option change type.
//!
//! Promoted from `reovim_driver_text_session::api::OptionChange`.

use reovim_kernel::api::v1::{OptionValue, WindowId};

/// Represents a single editor option change.
#[derive(Debug, Clone)]
pub struct OptionChange {
    /// Name of the option that changed.
    pub name: String,
    /// New value.
    pub value: OptionValue,
    /// Window ID if window-scoped, None if global.
    pub window_id: Option<WindowId>,
}

impl OptionChange {
    /// Create a global option change.
    #[must_use]
    pub fn global(name: impl Into<String>, value: OptionValue) -> Self {
        Self {
            name: name.into(),
            value,
            window_id: None,
        }
    }

    /// Create a window-scoped option change.
    #[must_use]
    pub fn window(name: impl Into<String>, value: OptionValue, window_id: WindowId) -> Self {
        Self {
            name: name.into(),
            value,
            window_id: Some(window_id),
        }
    }
}
