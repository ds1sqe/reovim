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
