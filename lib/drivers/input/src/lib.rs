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
//! lib/drivers/input/        <-- Canonical types (this crate)
//!        ^
//!        |  (platform code converts to canonical types)
//!        |
//! lib/arch/unix/            <-- crossterm → canonical
//! lib/arch/wasm/            <-- JS events → canonical [future]
//! lib/arch/android/         <-- Android events → canonical [future]
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
mod mode;
mod mouse;
mod resolver;
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
pub use mode::{KeySequence, Keybinding, ModeInput};

// Re-export fallback types (Phase 8 - Break editor->runner cycle)
pub use fallback::{
    BeepFallback, FallbackContext, FallbackResult, InputFallbackHandler, NoOpFallback,
};

// Re-export resolver types (Phase 9 - Flexible mode system)
pub use resolver::{
    ArgValue, ModeKeyResolver, ModeState, ModeTransition, PopResult, ResolveContext, ResolveResult,
    TransitionContext,
};
