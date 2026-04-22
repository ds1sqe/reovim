use {
    super::*,
    crate::{ClientModuleError, ProbeResult, Version, traits::ModuleContext},
};

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
    assert_eq!(module.id(), "custom-gutter");
    assert_eq!(module.domain_gutter_width(), 6);
}

#[test]
fn cell_grid_module_as_trait_object() {
    let module: Box<dyn CellGridClientModule> = Box::new(CustomGutterModule);
    assert_eq!(module.domain_gutter_width(), 6);
    assert_eq!(module.id(), "custom-gutter");
}
