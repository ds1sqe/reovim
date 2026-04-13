//! Breadcrumb statusline data provider.
//!
//! Produces scope context as a breadcrumb string for the statusline.

use reovim_subsys_statusline::{ComponentData, ComponentDataContext, ComponentDataProvider};

/// Statusline data provider that produces scope breadcrumbs.
///
/// Reads `ComponentDataContext::breadcrumb` field, which is populated by
/// the context bridge during the render cycle.
pub struct BreadcrumbComponent;

impl ComponentDataProvider for BreadcrumbComponent {
    fn id(&self) -> &'static str {
        "breadcrumb"
    }

    fn data(&self, ctx: &ComponentDataContext) -> ComponentData {
        match &ctx.breadcrumb {
            Some(text) if !text.is_empty() => ComponentData::new(format!(" {text} ")),
            _ => ComponentData::hidden(),
        }
    }
}

#[cfg(test)]
#[path = "breadcrumb_tests.rs"]
mod tests;
