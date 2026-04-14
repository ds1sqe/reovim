//! Per-client pair state.
//!
//! `PairState` is a `SessionExtension` that stores computed bracket
//! depths and the current matched pair for a client's active buffer.

use std::collections::HashMap;

use reovim_driver_text_session::SessionExtension;

use crate::{matched::MatchedPair, rainbow::BracketInfo};

/// User-configurable pair options.
#[derive(Debug, Clone)]
pub struct PairOptions {
    /// Enable rainbow bracket coloring.
    pub rainbow: bool,
    /// Enable auto-pair insertion.
    pub autopair: bool,
    /// Enable matched-pair highlighting.
    pub matchpair: bool,
}

impl Default for PairOptions {
    fn default() -> Self {
        Self {
            rainbow: true,
            autopair: true,
            matchpair: true,
        }
    }
}

/// Per-client pair state stored as a session extension.
pub struct PairState {
    /// Computed bracket positions and depths for the active buffer.
    pub brackets: HashMap<(usize, usize), BracketInfo>,
    /// The innermost matched pair around the cursor.
    pub matched: Option<MatchedPair>,
    /// User options.
    pub options: PairOptions,
}

impl SessionExtension for PairState {
    fn create() -> Self {
        Self {
            brackets: HashMap::new(),
            matched: None,
            options: PairOptions::default(),
        }
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
