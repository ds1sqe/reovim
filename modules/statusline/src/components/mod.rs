//! Built-in statusline components.
//!
//! Provides default implementations for common statusline components.
//!
//! # Built-in Components
//!
//! - [`ModeComponent`] - Current editor mode (NORMAL, INSERT, etc.)
//! - [`FilenameComponent`] - Buffer filename with modified/readonly indicators
//! - [`FiletypeComponent`] - Buffer filetype
//! - [`PositionComponent`] - Cursor position (line:col) with percentage
//!
//! # Extension Components
//!
//! These components demonstrate cross-module registration:
//!
//! - [`BranchComponent`] - Git branch (example for cached async data)
//! - [`DiagnosticsComponent`] - LSP diagnostics (example for context data)
//!
//! See each component's module documentation for registration examples.

mod branch;
mod diagnostics;
mod filename;
mod filetype;
mod mode;
mod position;

pub use {
    branch::BranchComponent, diagnostics::DiagnosticsComponent, filename::FilenameComponent,
    filetype::FiletypeComponent, mode::ModeComponent, position::PositionComponent,
};
