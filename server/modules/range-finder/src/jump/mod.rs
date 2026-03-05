//! Jump navigation sub-module.
//!
//! Provides two-char search (`s{char}{char}`) with label overlay.
//! Labels are assigned in home-row priority order and displayed at
//! match positions; selecting a label jumps the cursor there.

pub mod bridge;
pub mod command;
pub mod ids;
pub mod resolver;
pub mod search;
pub mod state;
