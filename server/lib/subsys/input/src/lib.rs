#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Input subsystem contracts for reovim — canonical input types and traits.
//!
//! Linux equivalent: `drivers/input/` + `include/linux/input.h`
//!
//! # Architecture
//!
//! This crate defines the **canonical input vocabulary** for all platforms.
//! Platform-specific code converts native events to these types.
//!
//! ```text
//! server/lib/subsys/input/  <-- Canonical types (this crate)
//!        ^
//!        |  (platform code converts to canonical types)
//!        |
//! shared/arch/unix/          <-- crossterm → canonical
//! shared/arch/wasm/          <-- JS events → canonical [future]
//! shared/arch/android/       <-- Android events → canonical [future]
//! ```
//!
//! # Components
//!
//! - **Types**: [`KeyCode`], [`KeyEvent`], [`Modifiers`], [`MouseEvent`]
//! - **Traits**: [`ClipboardProvider`], [`ModeLifecycleHandler`]
//! - **Conversions**: `From` impls between arch types and driver types

mod binding_info;
mod convert;
mod error;
pub mod input_event;
#[cfg(test)]
mod input_event_tests;
mod key;
mod keybinding_store;
mod lifecycle;
mod lookup;
mod lookup_policy_store;
mod mode;
mod mode_key;
mod mode_registry;
mod mode_store;
mod module_ext;
mod mouse;
mod provider;
mod traits;
mod transition;

// Re-export opaque input event types
pub use input_event::{
    INPUT_HEADER_SIZE, InputEvent, InputFlags, InputPayloadError, input_context, input_flags,
    input_kind,
};

// Re-export error types
pub use error::{ClipboardError, InputError};

// Re-export key types
pub use key::{KeyCode, KeyEvent, KeyEventKind, KeymapResult, Modifiers};

// Re-export mouse types
pub use mouse::{MouseButton, MouseEvent, MouseEventKind};

// Re-export traits
pub use traits::ClipboardProvider;

// Re-export mode types
pub use mode::{KeySequence, Keybinding, KeybindingTarget};

// Re-export lookup types
pub use lookup::{
    BindingLayer, EagerLookupPolicy, KeyLookupPolicy, KeyLookupResult, KeyLookupState, KeymapQuery,
};

// Re-export lifecycle types
pub use lifecycle::{ModeLifecycleHandler, NopLifecycleHandler};

// Re-export provider types
pub use {
    module_ext::DefaultModeProviderModule,
    provider::{DefaultModeProvider, ProviderPriority},
};

// Re-export typed key and registry
pub use {mode_key::ModeProviderKey, mode_registry::ModeProviderRegistry};

// Re-export mode store
pub use mode_store::{ModeInfo, ModeInfoStore};

// Re-export keybinding store
pub use keybinding_store::KeybindingStore;

// Re-export lookup policy store
pub use lookup_policy_store::LookupPolicyStore;

// Re-export binding metadata
pub use binding_info::BindingInfo;

// Re-export mode transition types
pub use transition::{ModeTransition, PopResult, TransitionContext};

#[cfg(test)]
mod binding_info_tests;
#[cfg(test)]
mod convert_tests;
#[cfg(test)]
mod error_tests;
#[cfg(test)]
mod key_tests;
#[cfg(test)]
mod keybinding_store_tests;
#[cfg(test)]
mod lifecycle_tests;
#[cfg(test)]
mod lookup_tests;
#[cfg(test)]
mod mode_key_tests;
#[cfg(test)]
mod mode_registry_tests;
#[cfg(test)]
mod mode_store_tests;
#[cfg(test)]
mod mode_tests;
#[cfg(test)]
mod mouse_tests;
#[cfg(test)]
mod provider_tests;
#[cfg(test)]
mod traits_tests;
#[cfg(test)]
mod transition_tests;
