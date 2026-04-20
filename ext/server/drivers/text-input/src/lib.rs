#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Text input driver façade for reovim.
//!
//! This crate owns the driver-private text-input implementation and explicitly
//! re-exports the shared contracts, typed key/mouse vocabulary, and
//! envelope/header surface consumed by text-input policy modules.

mod dispatch_provider;
mod fallback;
mod keybinding_store;
mod lifecycle;
mod lookup_policy_store;
mod mode;
mod mode_key;
mod mode_registry;
mod mode_store;
mod module_ext;
mod pending;
mod provider;
mod resolver;
mod resolver_registry;

// Re-export fallback types
pub use fallback::{
    BeepFallback, FallbackContext, FallbackResult, InputFallbackHandler, NoOpFallback,
};

// Re-export resolver types
pub use resolver::{
    ArgValue, InputTarget, ModeKeyResolver, ModeState, OperatorArgs, ResolveContext, ResolveInput,
    ResolveResult, SessionApi, SessionApiDyn,
};

// Re-export resolver registry
pub use resolver_registry::ResolverRegistry;

// Key dispatch provider (sub-plan 05 Phase 1)
pub use dispatch_provider::ResolverDispatchProvider;

// Re-export pending bindings (moved from subsys-input in sub-plan 02 Phase 0)
pub use pending::PendingBindings;

// Re-export driver-private mode/keybinding implementation.
pub use {
    keybinding_store::KeybindingStore,
    lifecycle::{ModeLifecycleHandler, NopLifecycleHandler},
    lookup_policy_store::LookupPolicyStore,
    mode::{Keybinding, KeybindingTarget},
    mode_key::ModeProviderKey,
    mode_registry::ModeProviderRegistry,
    mode_store::ModeInfoStore,
    module_ext::DefaultModeProviderModule,
    provider::{DefaultModeProvider, ProviderPriority},
};

// Re-export shared contracts.
pub use reovim_subsys_input_contracts::{
    BindingInfo, BindingLayer, EagerLookupPolicy, KeyLookupPolicy, KeyLookupResult, KeyLookupState,
    KeySequence, KeymapQuery, ModeInfo, ModeTransition, PopResult, ToKeyToken, TransitionContext,
};

// Re-export typed key/mouse types.
pub use reovim_input_codec::{
    KeyCode, KeyEvent, KeyEventKind, KeymapResult, Modifiers, MouseButton, MouseEvent,
    MouseEventKind,
};

// Re-export ExtensionMap from session (backwards compat)
pub use reovim_driver_text_session::ExtensionMap;

// Re-export envelope/header-only input surface.
pub use reovim_subsys_input::{
    INPUT_HEADER_SIZE, InputEvent, InputFlags, InputPayloadError, input_context, input_flags,
    input_kind,
};

#[cfg(test)]
mod fallback_tests;
#[cfg(test)]
mod keybinding_store_tests;
#[cfg(test)]
mod lifecycle_tests;
#[cfg(test)]
mod lookup_policy_store_tests;
#[cfg(test)]
mod mode_key_tests;
#[cfg(test)]
mod mode_registry_tests;
#[cfg(test)]
mod mode_store_tests;
#[cfg(test)]
mod mode_tests;
#[cfg(test)]
mod module_ext_tests;
#[cfg(test)]
mod pending_tests;
#[cfg(test)]
mod provider_tests;
#[cfg(test)]
mod resolver_registry_tests;
#[cfg(test)]
mod resolver_tests;
