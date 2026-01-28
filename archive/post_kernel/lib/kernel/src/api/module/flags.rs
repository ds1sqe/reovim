//! Registration capability flags.

/// Registration capability flags (Linux-inspired).
///
/// Like Linux kernel module flags, these control registration behavior.
/// Used by all registration types to indicate how the runner should handle
/// the registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)] // Flags struct intentionally uses multiple bools
pub struct RegistrationFlags {
    /// Registration is required (fail if cannot register).
    pub required: bool,
    /// Can be deferred if dependencies not ready (Linux: `-EPROBE_DEFER`).
    pub deferrable: bool,
    /// Should be registered early (before other modules).
    pub early: bool,
    /// Acts as fallback if no other handler matches.
    pub fallback: bool,
}

impl RegistrationFlags {
    /// Create default flags (all false).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            required: false,
            deferrable: false,
            early: false,
            fallback: false,
        }
    }

    /// Create flags for required registration.
    #[must_use]
    pub const fn required() -> Self {
        Self {
            required: true,
            deferrable: false,
            early: false,
            fallback: false,
        }
    }

    /// Create flags for deferrable registration.
    #[must_use]
    pub const fn deferrable() -> Self {
        Self {
            required: false,
            deferrable: true,
            early: false,
            fallback: false,
        }
    }

    // ========================================================================
    // Chainable Builders
    // ========================================================================

    /// Set the required flag (chainable).
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::api::v1::RegistrationFlags;
    ///
    /// let flags = RegistrationFlags::new().set_required().set_early();
    /// assert!(flags.is_required());
    /// assert!(flags.is_early());
    /// ```
    #[must_use]
    pub const fn set_required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Set the deferrable flag (chainable).
    #[must_use]
    pub const fn set_deferrable(mut self) -> Self {
        self.deferrable = true;
        self
    }

    /// Set the early flag (chainable).
    #[must_use]
    pub const fn set_early(mut self) -> Self {
        self.early = true;
        self
    }

    /// Set the fallback flag (chainable).
    #[must_use]
    pub const fn set_fallback(mut self) -> Self {
        self.fallback = true;
        self
    }

    // ========================================================================
    // Query Methods
    // ========================================================================

    /// Check if this registration is required.
    #[must_use]
    pub const fn is_required(&self) -> bool {
        self.required
    }

    /// Check if this registration is deferrable.
    #[must_use]
    pub const fn is_deferrable(&self) -> bool {
        self.deferrable
    }

    /// Check if this registration should happen early.
    #[must_use]
    pub const fn is_early(&self) -> bool {
        self.early
    }

    /// Check if this registration acts as a fallback.
    #[must_use]
    pub const fn is_fallback(&self) -> bool {
        self.fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registration_flags_default() {
        let flags = RegistrationFlags::new();
        assert!(!flags.required);
        assert!(!flags.deferrable);
        assert!(!flags.early);
        assert!(!flags.fallback);

        // Default trait should match new()
        let default_flags = RegistrationFlags::default();
        assert_eq!(flags, default_flags);
    }

    #[test]
    fn test_registration_flags_required() {
        let flags = RegistrationFlags::required();
        assert!(flags.required);
        assert!(!flags.deferrable);
        assert!(!flags.early);
        assert!(!flags.fallback);
    }

    #[test]
    fn test_registration_flags_deferrable() {
        let flags = RegistrationFlags::deferrable();
        assert!(!flags.required);
        assert!(flags.deferrable);
        assert!(!flags.early);
        assert!(!flags.fallback);
    }

    #[test]
    fn test_registration_flags_chainable_builders() {
        // Test chaining multiple flags
        let flags = RegistrationFlags::new().set_required().set_early();

        assert!(flags.required);
        assert!(!flags.deferrable);
        assert!(flags.early);
        assert!(!flags.fallback);

        // Test all flags together
        let all_flags = RegistrationFlags::new()
            .set_required()
            .set_deferrable()
            .set_early()
            .set_fallback();

        assert!(all_flags.required);
        assert!(all_flags.deferrable);
        assert!(all_flags.early);
        assert!(all_flags.fallback);
    }

    #[test]
    fn test_registration_flags_query_methods() {
        let flags = RegistrationFlags::new().set_required().set_early();

        assert!(flags.is_required());
        assert!(!flags.is_deferrable());
        assert!(flags.is_early());
        assert!(!flags.is_fallback());
    }
}
