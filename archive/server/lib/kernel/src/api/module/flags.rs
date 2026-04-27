//! Registration capability flags.

/// Bit position for "required" flag.
const FLAG_REQUIRED: u8 = 1 << 0;
/// Bit position for "deferrable" flag.
const FLAG_DEFERRABLE: u8 = 1 << 1;
/// Bit position for "early" flag.
const FLAG_EARLY: u8 = 1 << 2;
/// Bit position for "fallback" flag.
const FLAG_FALLBACK: u8 = 1 << 3;

/// Registration capability flags (Linux-inspired).
///
/// Like Linux kernel module flags, these control registration behavior.
/// Used by all registration types to indicate how the runner should handle
/// the registration.
///
/// Stored as a `u8` bitfield to avoid excessive boolean fields while
/// preserving zero external dependencies in the kernel crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RegistrationFlags(u8);

impl RegistrationFlags {
    /// Create default flags (all false).
    #[must_use]
    pub const fn new() -> Self {
        Self(0)
    }

    /// Create flags for required registration.
    #[must_use]
    pub const fn required() -> Self {
        Self(FLAG_REQUIRED)
    }

    /// Create flags for deferrable registration.
    #[must_use]
    pub const fn deferrable() -> Self {
        Self(FLAG_DEFERRABLE)
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
        self.0 |= FLAG_REQUIRED;
        self
    }

    /// Set the deferrable flag (chainable).
    #[must_use]
    pub const fn set_deferrable(mut self) -> Self {
        self.0 |= FLAG_DEFERRABLE;
        self
    }

    /// Set the early flag (chainable).
    #[must_use]
    pub const fn set_early(mut self) -> Self {
        self.0 |= FLAG_EARLY;
        self
    }

    /// Set the fallback flag (chainable).
    #[must_use]
    pub const fn set_fallback(mut self) -> Self {
        self.0 |= FLAG_FALLBACK;
        self
    }

    // ========================================================================
    // Query Methods
    // ========================================================================

    /// Check if this registration is required.
    #[must_use]
    pub const fn is_required(self) -> bool {
        self.0 & FLAG_REQUIRED != 0
    }

    /// Check if this registration is deferrable.
    #[must_use]
    pub const fn is_deferrable(self) -> bool {
        self.0 & FLAG_DEFERRABLE != 0
    }

    /// Check if this registration should happen early.
    #[must_use]
    pub const fn is_early(self) -> bool {
        self.0 & FLAG_EARLY != 0
    }

    /// Check if this registration acts as a fallback.
    #[must_use]
    pub const fn is_fallback(self) -> bool {
        self.0 & FLAG_FALLBACK != 0
    }
}
