//! Mode key resolver registry.
//!
//! This module provides the mechanism for registering and looking up
//! mode-specific key resolvers. The actual resolver implementations
//! (policy) live in policy modules like `modules/vim/`.
//!
//! # Note
//!
//! As of Epic #417, `ResolverRegistry` has been moved to `lib/drivers/input/`
//! since it's pure mechanism (storage/lookup). This module re-exports it
//! for backwards compatibility.

// Re-export from driver (Epic #417 - mechanism/policy separation)
pub use reovim_driver_text_input::ResolverRegistry;
