//! Unified UI Component System
//!
//! This module provides a unified architecture for all UI components in Reovim.
//! It combines the functionality of the previous `Interactor`, `DisplayComponent`,
//! and `Layer` traits into a single `UIComponent` trait.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Unified UIComponent System                    │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  Focusable (input-receiving)     │  Display-only (no input)     │
//! │  ─────────────────────────────   │  ────────────────────────     │
//! │  • EditorComponent               │  • StatusLineComponent        │
//! │  • ExplorerComponent             │  • TabLineComponent           │
//! │  • TelescopeComponent            │                               │
//! │  • SettingsComponent             │                               │
//! │  • CommandLineComponent          │                               │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Key Types
//!
//! - [`ComponentId`] - Unique identifier for components
//! - [`UIComponent`] - Unified trait for all UI components
//! - [`ComponentRegistry`] - Manages registered components and focus
//!
//! # Migration from Previous System
//!
//! The previous system had three separate traits:
//! - `Interactor` - for input-handling components
//! - `DisplayComponent` - for display-only components
//! - `Layer` - for z-ordered rendering
//!
//! The new `UIComponent` trait unifies all three:
//! - Input methods have default implementations (return `NotHandled`)
//! - Display-only components simply don't override input methods
//! - Z-ordering is handled via `z_order()` method
//!
//! # Example
//!
//! ```ignore
//! use reovim_core::ui_component::{ComponentId, UIComponent, ComponentRegistry};
//!
//! // Create a registry
//! let mut registry = ComponentRegistry::new();
//!
//! // Register components
//! registry.register(Box::new(EditorComponent::default()));
//! registry.register(Box::new(StatusLineComponent::new()));
//!
//! // Set focus to editor
//! registry.set_active(ComponentId::EDITOR);
//!
//! // Get the active component
//! let active = registry.active();
//! println!("Active: {}", active.display_name());
//! ```

mod component;
mod registry;

pub use {
    component::{ComponentId, UIComponent},
    registry::ComponentRegistry,
};

// Re-export InputResult from interactor for convenience
pub use crate::interactor::InputResult;
