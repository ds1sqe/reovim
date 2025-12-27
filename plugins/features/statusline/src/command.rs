//! Statusline plugin events
//!
//! Events for registering/unregistering sections from other plugins.

use reovim_core::{declare_event_command, event_bus::Event};

use crate::section::StatuslineSection;

/// Event to register a statusline section
///
/// Other plugins emit this to contribute sections to the status line.
#[derive(Clone)]
pub struct StatuslineSectionRegister {
    /// The section to register
    pub section: StatuslineSection,
}

impl Event for StatuslineSectionRegister {
    fn priority(&self) -> u32 {
        100
    }
}

impl std::fmt::Debug for StatuslineSectionRegister {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StatuslineSectionRegister")
            .field("section_id", &self.section.id)
            .finish()
    }
}

/// Event to unregister a statusline section
#[derive(Debug, Clone)]
pub struct StatuslineSectionUnregister {
    /// ID of the section to unregister
    pub id: &'static str,
}

impl Event for StatuslineSectionUnregister {
    fn priority(&self) -> u32 {
        100
    }
}

// Refresh command using unified pattern
declare_event_command! {
    StatuslineRefresh,
    id: "statusline_refresh",
    description: "Refresh the statusline",
}
