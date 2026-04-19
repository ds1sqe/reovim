#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Input subsystem contracts for reovim.
//!
//! Linux equivalent: `drivers/input/` + `include/linux/input.h`
//!
//! # Architecture
//!
//! Long-term, this crate owns only the opaque `InputEvent` envelope and frozen
//! header helpers. Mission #753 Plan 10 Phase 1 keeps the older typed key/mouse
//! and mode infrastructure definitions here temporarily for legacy consumers,
//! but they are compatibility overlap only — not canonical ownership.
//!
//! ```text
//! server/lib/subsys/input/   <-- Opaque InputEvent envelope + temporary legacy overlap
//! shared/input-codec/        <-- Canonical typed key/mouse vocab + arch conversions
//!        ^
//!        |  (platform code converts to canonical typed input here)
//!        |
//! shared/arch/unix/          <-- crossterm → typed input-codec types
//! shared/arch/wasm/          <-- JS events → typed input-codec types [future]
//! shared/arch/android/       <-- Android events → typed input-codec types [future]
//! ```
//!
//! # Components
//!
//! - **Envelope**: [`InputEvent`], [`InputFlags`], header accessors
//! - **Temporary legacy overlap**: typed key/mouse definitions only
//! - **Legacy auxiliary traits/errors**: clipboard/input compatibility while later
//!   purification phases are still in flight
//! - **Shared contract compatibility re-exports**: thin forwards into
//!   `reovim-subsys-input-contracts`

mod binding_info;
mod convert;
mod error;
pub mod input_event;
#[cfg(test)]
mod input_event_tests;
mod key;
mod lookup;
mod mouse;
mod traits;
mod transition;

// Re-export opaque input event types
pub use input_event::{
    INPUT_HEADER_SIZE, InputEvent, InputFlags, InputPayloadError, input_context, input_flags,
    input_kind,
};

// Re-export error types
pub use error::{ClipboardError, InputError};

// Temporary legacy overlap re-exports (Phase 1 compatibility only)
pub use key::{KeyCode, KeyEvent, KeyEventKind, KeymapResult, Modifiers};

// Temporary legacy overlap re-exports (Phase 1 compatibility only)
pub use mouse::{MouseButton, MouseEvent, MouseEventKind};

// Re-export traits
pub use traits::ClipboardProvider;

// Re-export lookup types
pub use lookup::{
    BindingLayer, EagerLookupPolicy, KeyLookupPolicy, KeyLookupResult, KeyLookupState, KeymapQuery,
};

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
mod lookup_tests;
#[cfg(test)]
mod mouse_tests;
#[cfg(test)]
mod traits_tests;
#[cfg(test)]
mod transition_tests;
