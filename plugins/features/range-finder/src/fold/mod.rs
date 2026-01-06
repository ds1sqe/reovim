//! Code folding subsystem for range-finder plugin

#![allow(dead_code)] // Temporary: will be used once wired up in lib.rs

pub mod command;
pub mod stage;
pub mod state;

// Re-export public types
#[allow(unused_imports)] // Temporary: will be used once wired up in lib.rs
pub use command::{FoldClose, FoldCloseAll, FoldOpen, FoldOpenAll, FoldRangesUpdated, FoldToggle};
#[allow(unused_imports)] // Temporary: will be used once wired up in lib.rs
pub use stage::FoldRenderStage;
pub use state::SharedFoldManager;
