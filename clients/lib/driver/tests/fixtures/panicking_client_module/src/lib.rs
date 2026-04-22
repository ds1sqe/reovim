//! Panicking client module fixture for #723 panic-containment tests.

use reovim_client_driver::{
    ClientModule, ClientModuleError, ModuleContext, ProbeResult, Rect, ChromeSurface, Version,
    traits::{PlatformCapabilities, ThemeProvider},
};

pub struct PanickingClientModule;

impl Default for PanickingClientModule {
    fn default() -> Self {
        Self::new()
    }
}

impl PanickingClientModule {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl ClientModule for PanickingClientModule {
    fn id(&self) -> &'static str {
        "panicking-client"
    }

    fn kind(&self) -> &'static str {
        "panicking-client"
    }

    fn name(&self) -> &'static str {
        "Panicking Client Module"
    }

    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ClientModuleError> {
        Ok(())
    }

    fn has_chrome(&self) -> bool {
        true
    }

    fn chrome_render(
        &self,
        _surface: &mut dyn ChromeSurface,
        _bounds: Rect,
        _caps: &dyn PlatformCapabilities,
    ) {
        panic!("intentional render panic from panicking fixture");
    }

    fn on_capabilities_changed(&mut self, _caps: &dyn PlatformCapabilities) {
        panic!("intentional capability-change panic from panicking fixture");
    }

    fn on_theme_changed(&mut self, _theme: &dyn ThemeProvider) {
        panic!("intentional theme-change panic from panicking fixture");
    }
}

reovim_module_macros::declare_client_module!(PanickingClientModule);
