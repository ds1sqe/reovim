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
mod tests {
    use super::*;
    use crate::{ClientModuleError, ProbeResult, Version, traits::ModuleContext};

    struct MinimalModule;

    impl ClientModule for MinimalModule {
        fn id(&self) -> &'static str {
            "minimal"
        }
        fn name(&self) -> &'static str {
            "Minimal"
        }
        fn version(&self) -> Version {
            Version::new(0, 1, 0)
        }
        fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
            ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), ClientModuleError> {
            Ok(())
        }
    }

    impl CellGridClientModule for MinimalModule {}

    struct CustomGutterModule;

    impl ClientModule for CustomGutterModule {
        fn id(&self) -> &'static str {
            "custom-gutter"
        }
        fn name(&self) -> &'static str {
            "Custom Gutter"
        }
        fn version(&self) -> Version {
            Version::new(0, 1, 0)
        }
        fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
            ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), ClientModuleError> {
            Ok(())
        }
    }

    impl CellGridClientModule for CustomGutterModule {
        fn domain_gutter_width(&self) -> u16 {
            6
        }
    }

    #[test]
    fn default_domain_gutter_width_is_zero() {
        let module = MinimalModule;
        assert_eq!(module.domain_gutter_width(), 0);
    }

    #[test]
    fn custom_domain_gutter_width() {
        let module = CustomGutterModule;
        assert_eq!(module.domain_gutter_width(), 6);
    }

    #[test]
    fn cell_grid_module_coexists_with_client_module() {
        let module = CustomGutterModule;
        // Access both ClientModule and CellGridClientModule methods.
        assert_eq!(module.id(), "custom-gutter");
        assert_eq!(module.domain_gutter_width(), 6);
    }

    #[test]
    fn cell_grid_module_as_trait_object() {
        let module: Box<dyn CellGridClientModule> = Box::new(CustomGutterModule);
        assert_eq!(module.domain_gutter_width(), 6);
        // ClientModule methods are accessible through supertrait.
        assert_eq!(module.id(), "custom-gutter");
    }
}
