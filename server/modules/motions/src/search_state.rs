//! Search state - per-session state for vim-style search.
//!
//! Re-exports `SearchState` from the session driver for use by motion commands.
//! The driver owns the type definition so both modules and runner can access it.

// Re-export SearchState from driver layer
pub use reovim_driver_session::api::SearchState;
