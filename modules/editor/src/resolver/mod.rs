//! Mode key resolver registry.
//!
//! This module provides the mechanism for registering and looking up
//! mode-specific key resolvers. The actual resolver implementations
//! (policy) live in policy modules like `modules/vim/`.

mod registry;

pub use registry::ResolverRegistry;
