//! Git signs annotation presenter.

use {crate::highlight::Style, reovim_arch::Color};

use super::{
    Annotation, AnnotationPresenter, ColumnWidth, KindPattern, PresentedOutput, PresenterContext,
};

/// Presenter that renders git signs as single-character gutter indicators.
///
/// - `git.add`: `+` (green)
/// - `git.change`: `~` (yellow)
/// - `git.delete`: `_` (red)
pub struct GitSignsPresenter;

impl GitSignsPresenter {
    /// Create a new git signs presenter.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for GitSignsPresenter {
    fn default() -> Self {
        Self::new()
    }
}

impl AnnotationPresenter for GitSignsPresenter {
    fn id(&self) -> &'static str {
        "git-signs"
    }

    fn handles(&self) -> KindPattern {
        KindPattern::prefix("git")
    }

    fn present(&self, annotation: &Annotation, _ctx: &PresenterContext) -> PresentedOutput {
        let name = annotation.kind.name();
        let (ch, color) = match name {
            "git.add" => ('+', Color::Green),
            "git.change" => ('~', Color::Yellow),
            "git.delete" => ('_', Color::Red),
            _ => return PresentedOutput::hidden(),
        };

        PresentedOutput::cell(ch, Style::default().fg(color))
    }

    fn column_width(&self, _ctx: &PresenterContext) -> ColumnWidth {
        ColumnWidth::fixed(1)
    }
}

#[cfg(test)]
#[path = "git_signs_presenter_tests.rs"]
mod tests;
