//! Modifier system for mode/filetype/state-based styling and behavior
//!
//! Modifiers allow conditional styling and behavior changes based on:
//! - Focus (Editor, Explorer, Telescope, etc.)
//! - Edit mode (Normal, Insert, Visual)
//! - Filetype (rust, python, etc.)
//! - Window state (active, modified, floating)

mod behavior;
mod context;
mod registry;
mod style;
mod traits;

pub use {
    behavior::{BehaviorModifiers, CommandId, FeatureFlags, KeyBindingAction, WindowBehaviorState},
    context::ModifierContext,
    registry::ModifierRegistry,
    style::{StyleModifiers, WindowDecorations, WindowStyleState},
    traits::{
        ActiveWindowModifier, FiletypeModifier, InsertModeModifier, ModifiedBufferModifier,
        Modifier, ModifierId, PredicateModifier, VisualModeModifier,
    },
};
