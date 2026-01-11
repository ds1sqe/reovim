//! Stable kernel API.
//!
//! Linux equivalent: `include/linux/`
//!
//! Defines the stable interface that drivers and modules can depend on.
//! Breaking changes require a new API version.

/// API version for compatibility checking.
pub const API_VERSION: (u32, u32, u32) = (0, 1, 0);
