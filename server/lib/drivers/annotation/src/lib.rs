//! Annotation data driver for the gutter annotation system.
//!
//! This crate provides the display-agnostic DATA types for the annotation
//! system. It contains no presentation logic (no `Style`, `Color`, `GutterCell`).
//!
//! # Architecture
//!
//! ```text
//! Server modules  ->  implement AnnotationSource  ->  produce data
//! Display driver  ->  implement AnnotationPresenter  ->  produce visuals
//!
//! This crate provides the shared data contract between them.
//! ```
//!
//! # Core Types
//!
//! - [`AnnotationKind`]: Hierarchical identifier (e.g., `"diagnostic.error"`)
//! - [`AnnotationTarget`]: Where the annotation applies (line, range, point, buffer)
//! - [`AnnotationPayload`]: Kind-specific data (number, text, severity, state)
//! - [`Annotation`]: The complete annotation with kind, target, priority, payload
//! - [`AnnotationSource`]: Trait for annotation data providers
//! - [`AnnotationStore`]: Efficient per-buffer annotation storage
//! - [`AnnotationSourceKey`]: Service registry key for annotation sources
//! - [`LineNumberMode`]: Display mode for line numbers
//!
//! # Data Flow
//!
//! ```text
//! Sources (server modules) -> Store -> [display boundary] -> Presenters -> Gutter Cells
//! ```
//!
//! # Example
//!
//! ```
//! use reovim_driver_annotation::{
//!     Annotation, AnnotationKind, AnnotationTarget, AnnotationPayload,
//! };
//!
//! // Create a diagnostic annotation
//! let diagnostic = Annotation {
//!     kind: AnnotationKind::new("diagnostic.error"),
//!     target: AnnotationTarget::Line(10),
//!     priority: 50,
//!     payload: AnnotationPayload::Severity(0), // 0 = error
//! };
//!
//! // Check if it affects a specific line
//! assert!(diagnostic.affects_line(10));
//! assert!(!diagnostic.affects_line(11));
//!
//! // Access the namespace
//! assert_eq!(diagnostic.kind.namespace(), Some("diagnostic"));
//! ```

mod line_number_mode;
mod registry;
mod source;
mod store;
mod types;

pub use {
    line_number_mode::LineNumberMode,
    registry::{AnnotationSourceKey, AnnotationSourceRegistry},
    source::{AnnotationContext, AnnotationSource},
    store::{AnnotationLayer, AnnotationStore, BufferAnnotationStore, SourceId},
    types::{Annotation, AnnotationKind, AnnotationPayload, AnnotationTarget},
};
