//! Generic line annotation system for gutter display.
//!
//! This module provides a unified architecture for displaying per-line
//! information in the gutter area, including line numbers, diagnostics,
//! git indicators, fold markers, and custom signs.
//!
//! # Architecture
//!
//! The annotation system follows mechanism/policy separation:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │  MODULES (Policy)                                               │
//! │  LineNumberSource, DiagnosticSource, GitSource, etc.            │
//! │  LineNumberPresenter, DiagnosticPresenter, etc.                 │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  ANNOTATION SUBSYS (Data) ← reovim-driver-annotation            │
//! │  Annotation, AnnotationKind, AnnotationTarget, AnnotationPayload│
//! │  AnnotationSource trait, AnnotationStore                        │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  DISPLAY DRIVER (Presentation) ← this module                    │
//! │  AnnotationPresenter trait, GutterComposer                      │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Core Types
//!
//! - [`AnnotationKind`]: Hierarchical identifier (e.g., `"diagnostic.error"`)
//! - [`AnnotationTarget`]: Where the annotation applies (line, range, point, buffer)
//! - [`AnnotationPayload`]: Kind-specific data (number, text, severity, state)
//! - [`Annotation`]: The complete annotation with kind, target, priority, payload
//!
//! ## Data Flow
//!
//! ```text
//! Sources → Store → Presenters → Composer → Gutter Cells
//!    ↑                   ↑
//! (Policy)          (Policy)
//! ```
//!
//! # Example
//!
//! ```
//! use reovim_driver_display::annotation::{
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

// Presenter implementations (moved from server modules in D2 Phase 3)
mod blame_presenter;
mod diagnostic_presenter;
mod git_signs_presenter;
mod line_number_presenter;

// Presentation-only modules (stay in display driver)
mod composer;
mod config;
mod integration;
mod presenter;
mod registry;

// Re-export data types from annotation driver (backward compatibility)
pub use reovim_driver_annotation::{
    Annotation, AnnotationContext, AnnotationKind, AnnotationLayer, AnnotationPayload,
    AnnotationSource, AnnotationSourceKey, AnnotationSourceRegistry, AnnotationStore,
    AnnotationTarget, BufferAnnotationStore, LineNumberMode, SourceId,
};

// Presentation types (local to display)
pub use {
    composer::{ComposedLine, ComposerBuilder, GutterComposer},
    config::{ColumnConfig, GutterConfig, VisibilityMode},
    integration::{GutterRenderer, GutterRendererKey, GutterRendererRegistry},
    presenter::{
        AnnotationPresenter, ColumnWidth, GutterCell, KindPattern, PresentedOutput,
        PresenterContext,
    },
    registry::PresenterRegistry,
};

// Concrete presenter implementations
pub use {
    blame_presenter::BlamePresenter, diagnostic_presenter::DiagnosticPresenter,
    git_signs_presenter::GitSignsPresenter, line_number_presenter::LineNumberPresenter,
};
