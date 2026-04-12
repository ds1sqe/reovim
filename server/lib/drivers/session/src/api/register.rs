//! Register access for yank/paste operations.
//!
//! This module provides the [`RegisterApi`] trait for register manipulation.
//! Commands use this to access and modify register contents.
//!
//! # Design
//!
//! Following Unix philosophy: this trait does ONE thing well - register access.
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_session::api::RegisterApi;
//!
//! fn paste_from_register<S: RegisterApi>(session: &S, register: Option<char>) {
//!     if let Some(content) = session.get_register(register) {
//!         // Use content.text and content.yank_type for paste
//!     }
//! }
//! ```

// Re-export text types for ergonomics
pub use reovim_domain_text::{RegisterContent, YankType};

/// Register access for yank/paste operations.
///
/// Provides access to vim-style registers for commands that need to
/// read or write yanked text.
///
/// # Register Names
///
/// - `None` - Unnamed register (default for yank/delete)
/// - `Some('a')` to `Some('z')` - Named registers
/// - `Some('A')` to `Some('Z')` - Append to named registers
/// - `Some('0')` to `Some('9')` - Numbered registers (yank history)
/// - `Some('+')` and `Some('*')` - System clipboard (driver-level)
pub trait RegisterApi: Send {
    /// Get register contents.
    ///
    /// Returns `None` if the register is empty or doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `name` - Register name, or `None` for the unnamed register
    fn get_register(&self, name: Option<char>) -> Option<RegisterContent>;

    /// Set register contents.
    ///
    /// Overwrites any existing content in the register.
    /// Use uppercase names (`'A'` to `'Z'`) to append instead.
    ///
    /// # Arguments
    ///
    /// * `name` - Register name, or `None` for the unnamed register
    /// * `content` - Content to store (text and yank type)
    fn set_register(&mut self, name: Option<char>, content: RegisterContent);
}
#[cfg(test)]
#[path = "tests/register.rs"]
mod tests;
