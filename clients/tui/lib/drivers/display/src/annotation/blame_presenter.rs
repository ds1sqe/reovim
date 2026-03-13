//! Blame annotation presenter.

use {crate::highlight::Style, reovim_arch::Color};

use super::{
    Annotation, AnnotationPresenter, ColumnWidth, KindPattern, PresentedOutput, PresenterContext,
};

/// Presenter that renders blame text in the gutter.
///
/// Shows the formatted blame string (hash, author, summary)
/// in a dim grey style.
pub struct BlamePresenter;

impl BlamePresenter {
    /// Create a new blame presenter.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for BlamePresenter {
    fn default() -> Self {
        Self::new()
    }
}

impl AnnotationPresenter for BlamePresenter {
    fn id(&self) -> &'static str {
        "blame"
    }

    fn handles(&self) -> KindPattern {
        KindPattern::prefix("blame")
    }

    fn present(&self, annotation: &Annotation, _ctx: &PresenterContext) -> PresentedOutput {
        annotation
            .payload
            .as_text()
            .map_or_else(PresentedOutput::hidden, |text| {
                let style = Style::default().fg(Color::DarkGrey);
                PresentedOutput::text(text, &style)
            })
    }

    fn column_width(&self, _ctx: &PresenterContext) -> ColumnWidth {
        ColumnWidth::dynamic(0, 60)
    }
}

#[cfg(test)]
#[path = "blame_presenter_tests.rs"]
mod tests;
