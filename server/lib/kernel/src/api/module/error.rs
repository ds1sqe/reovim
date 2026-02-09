//! Module error types.

use std::fmt;

use super::ModuleId;

/// Errors that can occur during module operations.
#[derive(Debug, Clone, PartialEq, Eq)]
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

// LLVM coverage artifact: match arm headers and closing braces in Display impl
// are marked DA:0 despite all variants being exercised in tests.
#[cfg_attr(coverage_nightly, coverage(off))]
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

/// Result of module probe/initialization.
///
/// Linux equivalent: Return value from `probe()` function.
/// - `Success` = success (Linux: `0`)
/// - `Defer` = try again later (Linux: `-EPROBE_DEFER`)
/// - `Failed` = permanent failure (Linux: negative errno)
///
/// # Example
///
/// ```ignore
/// fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
///     if !ctx.kernel.event_bus.has_subscriber("treesitter") {
///         return ProbeResult::Defer("waiting for treesitter".into());
///     }
///     ProbeResult::Success
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeResult {
    /// Probe succeeded, module is ready.
    Success,
    /// Probe deferred, retry later (with reason).
    /// Linux equivalent: `-EPROBE_DEFER`
    Defer(String),
    /// Probe failed permanently.
    Failed(ModuleError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
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

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_module_error_variants() {
        let err = ModuleError::LoadFailed("dlopen failed".into());
        assert!(format!("{err}").contains("load"));

        let err = ModuleError::NoEntryPoint("reovim_module".into());
        assert!(format!("{err}").contains("entry point"));

        let err = ModuleError::InitFailed("config missing".into());
        assert!(format!("{err}").contains("initialization failed"));

        let err = ModuleError::InUse {
            module: ModuleId::new("core"),
            by: ModuleId::new("lang-rust"),
        };
        assert!(format!("{err}").contains("in use"));

        let err = ModuleError::NotLoaded(ModuleId::new("missing"));
        assert!(format!("{err}").contains("not loaded"));
    }

    #[test]
    fn test_probe_result_success() {
        let result = ProbeResult::Success;
        assert_eq!(result, ProbeResult::Success);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_probe_result_defer() {
        let result = ProbeResult::Defer("waiting for treesitter".into());
        if let ProbeResult::Defer(reason) = &result {
            assert!(reason.contains("treesitter"));
        } else {
            panic!("expected Defer variant");
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_probe_result_failed() {
        let err = ModuleError::InitFailed("config missing".into());
        let result = ProbeResult::Failed(err.clone());
        if let ProbeResult::Failed(e) = result {
            assert_eq!(e, err);
        } else {
            panic!("expected Failed variant");
        }
    }
}
