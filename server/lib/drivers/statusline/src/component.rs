//! Display-agnostic component data provider.
//!
//! Server modules implement [`ComponentDataProvider`] to produce raw text
//! data for the statusline. The display driver wraps these with
//! presentation (Style, Color).

use reovim_kernel::api::v1::{MultiServiceRegistry, ServiceKey};

use super::ComponentDataContext;

/// Output from a data provider — text only, no Style.
///
/// The display driver's adapter converts this to `ComponentOutput`
/// (which adds `style: Option<Style>`).
#[derive(Debug, Clone)]
pub struct ComponentData {
    /// Rendered text content.
    pub text: String,
    /// Whether this component should be displayed.
    pub visible: bool,
    /// Minimum width hint (for fixed-width components like mode).
    pub min_width: Option<usize>,
    /// Priority for truncation (higher = truncate later, 0-255).
    pub truncation_priority: u8,
}

impl ComponentData {
    /// Create a new visible component data.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            visible: true,
            min_width: None,
            truncation_priority: 128,
        }
    }

    /// Create a hidden component data.
    #[must_use]
    pub const fn hidden() -> Self {
        Self {
            text: String::new(),
            visible: false,
            min_width: None,
            truncation_priority: 0,
        }
    }

    /// Conditionally show/hide based on a condition.
    #[must_use]
    pub const fn when(mut self, condition: bool) -> Self {
        self.visible = condition;
        self
    }

    /// Set minimum width.
    #[must_use]
    pub const fn with_min_width(mut self, width: usize) -> Self {
        self.min_width = Some(width);
        self
    }

    /// Set truncation priority (higher = truncate later).
    #[must_use]
    pub const fn with_priority(mut self, priority: u8) -> Self {
        self.truncation_priority = priority;
        self
    }
}

impl Default for ComponentData {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn default() -> Self {
        Self::hidden()
    }
}

/// Display-agnostic trait for statusline components.
///
/// Server modules implement this to produce raw text data.
/// The display driver wraps implementations with an adapter that
/// adds Style from the theme.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync`.
pub trait ComponentDataProvider: Send + Sync {
    /// Unique component identifier (e.g., "mode", "filename", "branch").
    fn id(&self) -> &'static str;

    /// Produce component data from current editor state.
    ///
    /// Should be fast — no I/O, no blocking.
    fn data(&self, ctx: &ComponentDataContext) -> ComponentData;
}

/// Key for data provider lookup in the registry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComponentDataProviderKey(String);

impl ComponentDataProviderKey {
    /// Create a new data provider key.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the component ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.0
    }
}

impl ServiceKey for ComponentDataProviderKey {
    fn service_name() -> &'static str {
        "ComponentDataProvider"
    }
}

/// Registry for cross-module data providers.
///
/// Server modules register [`ComponentDataProvider`] implementations
/// during their `init()` phase. The display driver reads this registry
/// and wraps each provider with a presentation adapter.
pub type ComponentDataProviderRegistry =
    MultiServiceRegistry<ComponentDataProviderKey, dyn ComponentDataProvider>;
