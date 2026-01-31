//! Command provider trait for module command registration.
//!
//! This trait allows modules to provide command handlers for registration
//! without modifying the kernel Module trait. It maintains kernel purity
//! by keeping driver-layer concerns in the driver layer.
//!
//! # Architecture
//!
//! ```text
//! Kernel Layer                    Driver Layer
//! ┌──────────────────┐           ┌─────────────────────┐
//! │ Module trait     │           │ CommandProvider     │
//! │ (identity, deps) │           │ (handlers)          │
//! └──────────────────┘           └─────────────────────┘
//!          │                               │
//!          └───────────┬───────────────────┘
//!                      │
//!                      ▼
//!              ┌───────────────┐
//!              │ EditorModule  │ implements both
//!              └───────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_command::{CommandHandler, CommandProvider};
//!
//! struct EditorModule;
//!
//! impl CommandProvider for EditorModule {
//!     fn command_handlers(&self) -> Vec<Box<dyn CommandHandler>> {
//!         vec![
//!             Box::new(CursorUp),
//!             Box::new(CursorDown),
//!             // ...
//!         ]
//!     }
//! }
//! ```

use super::CommandHandler;

/// Trait for modules that provide command handlers.
///
/// Modules implement this trait to register their command handlers.
/// The runner queries this trait during module loading to wire commands.
///
/// # Design
///
/// This trait is separate from the kernel's `Module` trait to maintain
/// kernel purity. The kernel defines module identity and lifecycle;
/// command handlers are a driver-layer concern.
///
/// # Thread Safety
///
/// The trait requires `Send + Sync` because modules may be accessed
/// from multiple threads during command registration and execution.
///
/// # Example
///
/// ```ignore
/// use reovim_driver_command::{CommandHandler, CommandProvider};
///
/// struct EditorModule;
///
/// impl CommandProvider for EditorModule {
///     fn command_handlers(&self) -> Vec<Box<dyn CommandHandler>> {
///         vec![
///             Box::new(CursorUp),
///             Box::new(CursorDown),
///             Box::new(DeleteChar),
///         ]
///     }
/// }
/// ```
pub trait CommandProvider: Send + Sync {
    /// Get command handlers to register.
    ///
    /// Returns boxed handlers that the runner will register in the
    /// command registry. Each handler must implement `CommandHandler`.
    ///
    /// # Implementation Notes
    ///
    /// - Return a fresh `Vec` each time (handlers are moved into the registry)
    /// - Use `Box::new()` for each command implementation
    /// - Commands should use `CommandId` constants from the module's `ids` module
    fn command_handlers(&self) -> Vec<Box<dyn CommandHandler>>;
}
