//! TUI-specific extension trait for cell-grid rendering.
//!
//! [`CellGridClientModule`] extends [`ClientModule`] with methods that only
//! make sense in a cell-grid (terminal/TUI) context. Other platforms (web,
//! native) use `ClientModule` directly without this extension.

use super::ClientModule;

/// TUI-specific extension of [`ClientModule`].
///
/// Modules that provide cell-grid domain rendering implement this trait
/// in addition to `ClientModule`. The TUI compositor uses these methods
/// for grid-specific layout calculations.
///
/// This trait is intentionally narrow — it contains only methods where
/// the cell-grid abstraction leaks into the module API. Methods that
/// work across all rendering models stay on `ClientModule`.
pub trait CellGridClientModule: ClientModule {
    /// Width of the domain-specific gutter in cell columns.
    ///
    /// This is the gutter width that a domain view module requests for
    /// its rendering area (e.g., line numbers in a text domain, layer
    /// indices in a 3D domain). Separate from annotation gutters which
    /// are per-module.
    ///
    /// Default: 0 (no domain gutter).
    fn domain_gutter_width(&self) -> u16 {
        0
    }
}

#[cfg(test)]
#[path = "cell_grid_tests.rs"]
mod tests;
