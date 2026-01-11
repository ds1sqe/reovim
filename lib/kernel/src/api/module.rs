//! Module identity and error types.
//!
//! Linux equivalent: `include/linux/module.h`

use std::fmt;

/// Unique identifier for a loadable module.
///
/// Convention: Use kebab-case names like "lang-rust", "feat-completion".
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleId(pub &'static str);

impl ModuleId {
    /// Create a new module identifier.
    #[must_use]
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }

    /// Get the identifier string.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

impl fmt::Display for ModuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Errors that can occur during module operations.
#[derive(Debug)]
pub enum ModuleError {
    /// Failed to load the shared object file.
    LoadFailed(String),

    /// Module does not export the required entry point.
    NoEntryPoint(String),

    /// Module initialization failed.
    InitFailed(String),

    /// Module API version is incompatible with kernel.
    IncompatibleVersion {
        /// Version the module requires.
        module: (u32, u32),
        /// Version the kernel provides.
        kernel: (u32, u32),
    },

    /// Module is in use by another module.
    InUse {
        /// The module that cannot be unloaded.
        module: ModuleId,
        /// The module that depends on it.
        by: ModuleId,
    },

    /// Module is not currently loaded.
    NotLoaded(ModuleId),

    /// Module file was not found.
    NotFound(String),
}

impl fmt::Display for ModuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LoadFailed(msg) => write!(f, "failed to load module: {msg}"),
            Self::NoEntryPoint(msg) => write!(f, "module missing entry point: {msg}"),
            Self::InitFailed(msg) => write!(f, "module initialization failed: {msg}"),
            Self::IncompatibleVersion { module, kernel } => {
                write!(
                    f,
                    "module requires API {}.{}, kernel provides {}.{}",
                    module.0, module.1, kernel.0, kernel.1
                )
            }
            Self::InUse { module, by } => {
                write!(f, "module '{module}' is in use by '{by}'")
            }
            Self::NotLoaded(id) => write!(f, "module '{id}' is not loaded"),
            Self::NotFound(name) => write!(f, "module '{name}' not found"),
        }
    }
}

impl std::error::Error for ModuleError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        let id = ModuleId::new("lang-rust");
        assert_eq!(id.as_str(), "lang-rust");
        assert_eq!(format!("{id}"), "lang-rust");
    }

    #[test]
    fn test_module_id_equality() {
        let id1 = ModuleId::new("lang-rust");
        let id2 = ModuleId::new("lang-rust");
        let id3 = ModuleId::new("lang-python");
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_module_error_display() {
        let err = ModuleError::NotFound("test-module".into());
        assert_eq!(format!("{err}"), "module 'test-module' not found");

        let err = ModuleError::IncompatibleVersion {
            module: (2, 0),
            kernel: (1, 5),
        };
        assert!(format!("{err}").contains("2.0"));
        assert!(format!("{err}").contains("1.5"));
    }
}
