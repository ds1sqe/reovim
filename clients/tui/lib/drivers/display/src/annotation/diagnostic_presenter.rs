//! Diagnostic annotation presenter.
//!
//! Renders diagnostic severity as Nerd Font icons in the gutter:
//! - Error (severity 0): red icon
//! - Warning (severity 1): yellow icon
//! - Info (severity 2): blue icon
//! - Hint (severity 3): cyan icon

use {crate::highlight::Style, reovim_arch::Color};

use super::{
    Annotation, AnnotationPayload, AnnotationPresenter, ColumnWidth, KindPattern, PresentedOutput,
    PresenterContext,
};

/// Nerd Font diagnostic icon chars (extracted from `ui_icons`).
const ERROR_ICON: char = '\u{f015a}'; // 󰅚
const WARNING_ICON: char = '\u{f002a}'; // 󰀪
const INFO_ICON: char = '\u{f02fd}'; // 󰋽
const HINT_ICON: char = '\u{f0336}'; // 󰌶

/// Presenter that renders diagnostic severity icons in the gutter.
///
/// Maps `AnnotationPayload::Severity` values to colored Nerd Font icons.
/// Falls back to ASCII indicators when the payload is not a severity.
pub struct DiagnosticPresenter;

impl DiagnosticPresenter {
    /// Create a new diagnostic presenter.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for DiagnosticPresenter {
    fn default() -> Self {
        Self::new()
    }
}

impl AnnotationPresenter for DiagnosticPresenter {
    fn id(&self) -> &'static str {
        "diagnostics"
    }

    fn handles(&self) -> KindPattern {
        KindPattern::prefix("diagnostic")
    }

    fn present(&self, annotation: &Annotation, _ctx: &PresenterContext) -> PresentedOutput {
        let (icon, color) = match annotation.payload {
            AnnotationPayload::Severity(1) => (WARNING_ICON, Color::Yellow),
            AnnotationPayload::Severity(2) => (INFO_ICON, Color::Blue),
            AnnotationPayload::Severity(3) => (HINT_ICON, Color::Cyan),
            // Severity 0 (error) and unknown severities default to error icon
            AnnotationPayload::Severity(_) => (ERROR_ICON, Color::Red),
            _ => return PresentedOutput::hidden(),
        };

        PresentedOutput::cell(icon, Style::default().fg(color))
    }

    fn column_width(&self, _ctx: &PresenterContext) -> ColumnWidth {
        ColumnWidth::fixed(1)
    }
}

#[cfg(test)]
#[path = "diagnostic_presenter_tests.rs"]
mod tests;
