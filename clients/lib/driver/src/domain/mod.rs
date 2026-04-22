//! Domain view modules for client-side rendering.
//!
//! Each domain (text, mesh, image, etc.) has a view module that implements
//! [`ClientModule`](crate::ClientModule) with domain-specific rendering.
//! The compositor dispatches projections to modules based on
//! [`ClientModule::domain_id`].

mod text_view;

pub use text_view::TextDomainView;

use reovim_subsys_coordination::DomainId;

/// Render a placeholder for an unknown domain.
///
/// Called by the compositor when a projection arrives for a domain that
/// no loaded view module claims. Logs a warning and renders a visible
/// placeholder in the given surface region.
pub fn render_unknown_domain_placeholder(
    surface: &mut dyn crate::ChromeSurface,
    bounds: crate::Rect,
    domain_id: DomainId,
) {
    tracing::warn!(domain_id = domain_id.0, "no view module for domain");
    let style = crate::Style::new().fg(crate::types::Color::DarkGrey);
    let label = format!("[unknown domain {}]", domain_id.0);
    if bounds.width > 0 && bounds.height > 0 {
        surface.write_styled(bounds.x, bounds.y, &label, style);
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
