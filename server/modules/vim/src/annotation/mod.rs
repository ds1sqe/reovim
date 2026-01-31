//! Vim annotation module.
//!
//! Provides Vim-specific annotation sources and presenters for the
//! generic annotation system.
//!
//! # Architecture
//!
//! This module implements POLICY for the annotation system:
//! - Sources generate annotations (what data to show)
//! - Presenters render annotations (how to display them)
//!
//! The annotation system MECHANISM lives in the display driver.
//!
//! # Components
//!
//! - [`LineNumberSource`]: Generates line number annotations
//! - [`LineNumberPresenter`]: Renders line numbers to gutter cells

mod line_number;

pub use line_number::{
    LineNumberPresenter, LineNumberSource, create_line_number_presenter, create_line_number_source,
};
