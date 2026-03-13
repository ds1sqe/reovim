//! Vim annotation module.
//!
//! Provides Vim-specific annotation sources for the generic annotation system.
//!
//! # Architecture
//!
//! This module implements POLICY for the annotation system:
//! - Sources generate annotations (what data to show)
//!
//! Presenters (how to display annotations) live in the display driver.
//!
//! # Components
//!
//! - [`LineNumberSource`]: Generates line number annotations

mod line_number;

pub use line_number::{LineNumberSource, create_line_number_source};
