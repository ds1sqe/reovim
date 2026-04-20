//! Test module context builder.

use std::sync::Arc;

use crate::{ModuleContext, services::ClientServiceRegistry};

use super::{MockModuleRegistry, MockPlatformCapabilities, MockServerHandle, MockThemeProvider};

/// Owned test context that produces borrowed `ModuleContext` references.
///
/// Use the builder to configure mock components, then call `as_context()`
/// to get a `ModuleContext<'_>` suitable for passing to `ClientModule::init()`.
pub struct TestModuleContext {
    caps: MockPlatformCapabilities,
    server: Arc<MockServerHandle>,
    theme: MockThemeProvider,
    services: Option<ClientServiceRegistry>,
    registry: Option<MockModuleRegistry>,
}

impl TestModuleContext {
    /// Start building a test context.
    #[must_use]
    pub fn builder() -> TestModuleContextBuilder {
        TestModuleContextBuilder::default()
    }

    /// Borrow as a `ModuleContext` suitable for `ClientModule::init()`.
    #[must_use]
    pub fn as_context(&self) -> ModuleContext<'_> {
        ModuleContext {
            capabilities: &self.caps,
            server: Arc::clone(&self.server) as Arc<dyn crate::ServerHandle>,
            theme: &self.theme,
            services: self.services.as_ref(),
            module_registry: self
                .registry
                .as_ref()
                .map(|r| r as &dyn crate::ClientModuleRegistry),
        }
    }

    /// Access the mock capabilities directly.
    #[must_use]
    pub const fn capabilities(&self) -> &MockPlatformCapabilities {
        &self.caps
    }

    /// Access the mock server handle directly.
    #[must_use]
    pub fn server(&self) -> &MockServerHandle {
        &self.server
    }

    /// Access the mock theme provider directly.
    #[must_use]
    pub const fn theme(&self) -> &MockThemeProvider {
        &self.theme
    }
}

/// Builder for `TestModuleContext`.
#[derive(Default)]
pub struct TestModuleContextBuilder {
    caps: MockPlatformCapabilities,
    server: MockServerHandle,
    theme: MockThemeProvider,
    services: Option<ClientServiceRegistry>,
    registry: Option<MockModuleRegistry>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl TestModuleContextBuilder {
    /// Set grid size on the mock capabilities.
    #[must_use]
    pub const fn grid_size(mut self, width: u16, height: u16) -> Self {
        self.caps = self.caps.with_grid_size(width, height);
        self
    }

    /// Set dark mode on the mock capabilities.
    #[must_use]
    pub const fn dark_mode(mut self, dark: bool) -> Self {
        self.caps = self.caps.with_dark_mode(dark);
        self
    }

    /// Seed server options.
    #[must_use]
    pub fn server_options(
        mut self,
        options: std::collections::HashMap<String, crate::OptionValue>,
    ) -> Self {
        self.server = self.server.with_options(options);
        self
    }

    /// Seed a theme highlight.
    #[must_use]
    pub fn highlight(mut self, group: &str, style: crate::Style) -> Self {
        self.theme = self.theme.with_highlight(group, style);
        self
    }

    /// Seed running modules for the registry.
    #[must_use]
    pub fn running_modules(mut self, kinds: &[&str]) -> Self {
        self.registry = Some(MockModuleRegistry::new().with_running(kinds));
        self
    }

    /// Provide a service registry.
    #[must_use]
    pub fn services(mut self, registry: ClientServiceRegistry) -> Self {
        self.services = Some(registry);
        self
    }

    /// Build the test context.
    #[must_use]
    pub fn build(self) -> TestModuleContext {
        TestModuleContext {
            caps: self.caps,
            server: Arc::new(self.server),
            theme: self.theme,
            services: self.services,
            registry: self.registry,
        }
    }
}
