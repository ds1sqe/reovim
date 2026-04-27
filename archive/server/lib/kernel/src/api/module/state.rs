//! Module state and info types.

use std::fmt;

use {
    super::{Module, ModuleId},
    crate::api::version::Version,
};

/// Module lifecycle state.
///
/// State machine:
/// ```text
/// Loaded -> Initializing -> Running -> Unloading -> (removed)
///    |           |            |           |
///    +---------->+--->--------+--->-------+---> Failed
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ModuleState {
    /// Module is loaded but not initialized.
    #[default]
    Loaded,
    /// Module is currently initializing.
    Initializing,
    /// Module is running normally.
    Running,
    /// Module is being unloaded.
    Unloading,
    /// Module failed (with error message).
    Failed(String),
}

impl ModuleState {
    /// Check if transition to another state is valid.
    #[must_use]
    pub const fn can_transition_to(&self, next: &Self) -> bool {
        // Any state can transition to Failed
        if matches!(next, Self::Failed(_)) {
            return true;
        }

        matches!(
            (self, next),
            (Self::Loaded, Self::Initializing)
                | (Self::Initializing, Self::Running)
                | (Self::Running, Self::Unloading)
        )
    }
}

impl fmt::Display for ModuleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Loaded => write!(f, "loaded"),
            Self::Initializing => write!(f, "initializing"),
            Self::Running => write!(f, "running"),
            Self::Unloading => write!(f, "unloading"),
            Self::Failed(msg) => write!(f, "failed: {msg}"),
        }
    }
}

/// Static module metadata.
///
/// Used for module discovery and display.
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    /// Unique identifier.
    pub id: ModuleId,
    /// Human-readable name.
    pub name: &'static str,
    /// Version.
    pub version: Version,
    /// Required API version.
    pub api_version: Version,
    /// Description.
    pub description: &'static str,
    /// Author(s).
    pub authors: &'static [&'static str],
    /// License.
    pub license: &'static str,
}

impl ModuleInfo {
    /// Create module info from a Module instance.
    #[must_use]
    pub fn from_module<M: Module>(module: &M) -> Self {
        Self {
            id: module.id(),
            name: module.name(),
            version: module.version(),
            api_version: module.api_version(),
            description: "",
            authors: &[],
            license: "",
        }
    }
}
