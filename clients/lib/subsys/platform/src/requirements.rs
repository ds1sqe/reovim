//! Platform requirement declaration contract for client modules.
//!
//! Modules declare their required capabilities and feature flags via a
//! `const REQUIRES: Requirements` associated constant. The platform loader
//! reads this at load time to filter incompatible modules without
//! instantiating them.

use reovim_client_subsys_capability::{CapabilityId, FeatureFlag};

// =============================================================================
// Requirements
// =============================================================================

/// Platform requirements declared by a client module.
///
/// A module declares `const REQUIRES: Requirements = Requirements::new(...)`.
/// The platform loader reads this to determine whether the platform satisfies
/// the module's declared dependencies.
///
/// Both slices are `&'static` so the requirements can be embedded in module
/// binaries as zero-copy read-only data. This avoids heap allocation during
/// the loader's capability-match phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Requirements {
    /// Capabilities the platform must provide for this module to function.
    pub caps: &'static [CapabilityId],
    /// Feature flags the platform must expose.
    pub features: &'static [FeatureFlag],
}

impl Requirements {
    /// Construct requirements from static capability and feature slices.
    ///
    /// Use this in `const REQUIRES` declarations:
    ///
    /// ```rust
    /// use reovim_client_subsys_capability::{CapabilityId, FeatureFlag};
    /// use reovim_client_subsys_platform::Requirements;
    ///
    /// const CELL_CAP: CapabilityId = CapabilityId(1);
    /// const REQUIRES: Requirements = Requirements::new(&[CELL_CAP], &[]);
    /// ```
    #[must_use]
    pub const fn new(caps: &'static [CapabilityId], features: &'static [FeatureFlag]) -> Self {
        Self { caps, features }
    }

    /// Construct empty requirements (no capabilities or feature flags needed).
    ///
    /// Convenience for modules that run on any platform.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            caps: &[],
            features: &[],
        }
    }

    /// Returns true if this requirements set declares no requirements.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.caps.is_empty() && self.features.is_empty()
    }
}

// =============================================================================
// Platform trait
// =============================================================================

/// Abstract contract for the client-side platform runtime.
///
/// `Debug + Send + Sync` bounds let implementors be stored in
/// `Arc<dyn Platform>` across threads without unsafe.
pub trait Platform: std::fmt::Debug + Send + Sync {
    /// Human-readable platform name for logging and diagnostics.
    fn name(&self) -> &'static str;
}

#[cfg(test)]
#[path = "requirements_tests.rs"]
mod tests;
