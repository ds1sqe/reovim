//! Sample client module fixture for #729 end-to-end validation.

use reovim_client_driver::{
    ChromePosition, ClientModule, ClientModuleError, ModuleContext, PlatformCapabilities,
    ProbeResult, Rect, RenderSurface, Style, Version,
};

pub struct SampleClientModule;

impl SampleClientModule {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for SampleClientModule {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientModule for SampleClientModule {
    fn id(&self) -> &'static str {
        "sample-client"
    }

    fn kind(&self) -> &'static str {
        "sample-client"
    }

    fn name(&self) -> &'static str {
        "Sample Client Module"
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

    fn chrome_position(&self) -> ChromePosition {
        ChromePosition::Bottom
    }

    fn chrome_requested_size(&self, _caps: &dyn PlatformCapabilities) -> u16 {
        1
    }

    fn chrome_render(
        &self,
        surface: &mut dyn RenderSurface,
        bounds: Rect,
        _caps: &dyn PlatformCapabilities,
    ) {
        surface.fill(bounds, ' ', Style::default());
        let _ = surface.write_styled(bounds.x, bounds.y, "sample-panel", Style::default());
    }
}

reovim_module_macros::declare_client_module!(SampleClientModule);
