#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Input driver for reovim — resolver and fallback layer.
//!
//! Core input types and traits are in [`reovim_subsys_input`]. This crate
//! provides the domain-specific resolver system and fallback handlers.

mod fallback;
mod resolver;
mod resolver_registry;

// Re-export fallback types
pub use fallback::{
    BeepFallback, FallbackContext, FallbackResult, InputFallbackHandler, NoOpFallback,
};

// Re-export resolver types
pub use resolver::{
    ArgValue, InputTarget, ModeKeyResolver, ModeState, ModeTransition, OperatorArgs, PopResult,
    ResolveContext, ResolveInput, ResolveResult, SessionApi, SessionApiDyn, TransitionContext,
};

// Re-export resolver registry
pub use resolver_registry::ResolverRegistry;

// Re-export ExtensionMap from session (backwards compat)
pub use reovim_driver_session::ExtensionMap;

// Re-export everything from subsys-input for backwards compatibility
pub use reovim_subsys_input::*;

#[cfg(test)]
mod fallback_tests;
#[cfg(test)]
mod resolver_registry_tests;
#[cfg(test)]
mod resolver_tests;
