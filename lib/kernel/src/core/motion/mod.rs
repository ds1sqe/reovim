//! Motion calculations for cursor movement.
//!
//! This module provides pure motion calculations without any side effects.
//! The `MotionEngine` calculates target positions given a buffer, cursor, and motion.
//!
//! # Design Philosophy
//!
//! This follows the Linux kernel "mechanism, not policy" principle:
//! - The kernel provides *how* to calculate motions (mechanisms)
//! - Modules decide *what* keys trigger which motions (policies)

mod engine;
mod types;

pub use engine::MotionEngine;
pub use types::Motion;
