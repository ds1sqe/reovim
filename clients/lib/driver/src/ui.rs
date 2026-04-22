//! Unicode-aware text utilities — compatibility re-export.
//!
//! All utilities are now canonical in `reovim-client-subsys-chrome::ui`.
//! This module is a thin re-export shim preserved until Phase F decommissions driver.
//!
//! TODO(#753): Phase F removes this file entirely.

pub use reovim_client_subsys_chrome::ui::{display_width, truncate_end, truncate_start};
