#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Input driver for reovim - canonical input types and traits.
//!
//! Linux equivalent: `drivers/input/` + `include/linux/input.h`
//!
//! # Architecture
//!
//! This crate defines the **canonical input vocabulary** for all platforms.
//! Platform-specific code converts native events to these types.
//!
//! ```text
//! server/lib/drivers/input/  <-- Canonical types (this crate)
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
//! - **Traits**: [`InputDriver`], [`KeyHandler`], [`KeymapRegistry`], [`ClipboardProvider`]
//! - **Conversions**: `From` impls between arch types and driver types
//!
//! # Example
//!
//! ```
//! use reovim_driver_input::{KeyCode, KeyEvent, Modifiers, KeymapResult};
//!
//! // Create a simple key event
//! let key = KeyEvent::new(KeyCode::Char('j'));
//! assert!(key.is_press());
//!
//! // Create a key event with modifiers
//! let ctrl_s = KeyEvent::with_modifiers(KeyCode::Char('s'), Modifiers::CTRL);
//! assert!(ctrl_s.modifiers.contains(Modifiers::CTRL));
//! ```
//!
//! # Converting from arch types
//!
//! ```
//! use reovim_driver_input::KeyEvent;
//!
//! // Convert from arch KeyEvent to driver KeyEvent
//! let arch_event = reovim_arch::KeyEvent::new(reovim_arch::KeyCode::Char('x'));
//! let driver_event: KeyEvent = arch_event.into();
//! ```

mod convert;
mod error;
mod fallback;
mod key;
mod keybinding_store;
mod lifecycle;
mod lookup;
mod mode;
mod mode_key;
mod mode_registry;
mod mode_store;
mod module_ext;
mod mouse;
mod pending;
mod provider;
mod resolver;
mod resolver_registry;
mod traits;

// Re-export error types
pub use error::{ClipboardError, InputError};

// Re-export key types
pub use key::{KeyCode, KeyEvent, KeyEventKind, KeymapResult, Modifiers};

// Re-export mouse types
pub use mouse::{MouseButton, MouseEvent, MouseEventKind};

// Re-export traits
pub use traits::{
    ClipboardProvider, HandlerPriority, InputDriver, KeyHandler, KeyHandlerResult, KeymapRegistry,
    priority,
};

// Re-export mode types (Phase 6 - Kernel-driver architecture)
pub use mode::{KeySequence, Keybinding, KeybindingTarget};

// Re-export fallback types (Phase 8 - Break editor->runner cycle)
pub use fallback::{
    BeepFallback, FallbackContext, FallbackResult, InputFallbackHandler, NoOpFallback,
};

// Re-export resolver types (Phase 9 - Flexible mode system)
pub use resolver::{
    ArgValue, InputTarget, ModeKeyResolver, ModeState, ModeTransition, OperatorArgs, PopResult,
    ResolveContext, ResolveInput, ResolveResult, SessionApi, SessionApiDyn, TransitionContext,
};

// Re-export lookup types (Epic #353 - Mechanism/Policy separation)
pub use lookup::{
    BindingLayer, EagerLookupPolicy, KeyLookupPolicy, KeyLookupResult, KeyLookupState, KeymapQuery,
    VimLookupPolicy,
};

// Re-export lifecycle types (Epic #372 - Mode Ownership)
pub use lifecycle::{ModeLifecycleHandler, NopLifecycleHandler};

// Re-export session extension types (Epic #385 - Server Simplification)
// Needed for ModeKeyResolver::resolve_with_extensions()
pub use reovim_driver_session::ExtensionMap;

// Re-export provider types (Epic #415 - Module provider hooks)
pub use {
    module_ext::DefaultModeProviderModule,
    provider::{DefaultModeProvider, ProviderPriority},
};

// Re-export typed key and registry (Epic #417 - UniqueProvider abstraction)
pub use {mode_key::ModeProviderKey, mode_registry::ModeProviderRegistry};

// Re-export resolver registry (Epic #417 - moved from modules/editor)
pub use resolver_registry::ResolverRegistry;

// Re-export mode store (Epic #417 Part 3 - module self-registration)
pub use mode_store::{ModeInfo, ModeInfoStore};

// Re-export keybinding store (Epic #417 Part 3 - module self-registration)
pub use keybinding_store::KeybindingStore;

// Re-export pending bindings extension (#468 - Which-Key bridge)
pub use pending::PendingBindings;
