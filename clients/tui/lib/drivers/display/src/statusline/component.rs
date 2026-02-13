//! Statusline component API.
//!
//! Defines the component trait and context types for building statusline content.
//!
//! # Architecture
//!
//! Components are pluggable units that render portions of the statusline.
//! Each component receives a [`ComponentContext`] snapshot of editor state
//! and returns a [`ComponentOutput`] with rendered text.
//!
//! # Example
//!
//! ```ignore
//! struct ModeComponent;
//!
//! impl ComponentProvider for ModeComponent {
//!     fn id(&self) -> &'static str { "mode" }
//!
//!     fn render(&self, ctx: &ComponentContext) -> ComponentOutput {
//!         ComponentOutput::new(format!(" {} ", ctx.mode))
//!     }
//! }
//! ```

use reovim_kernel::api::v1::{MultiServiceRegistry, ServiceKey};

use crate::Style;

/// Immutable snapshot of editor state for component rendering.
///
/// Created once per render cycle, shared by all components.
/// Components should not perform I/O or blocking operations.
#[derive(Debug, Clone, Default)]
pub struct ComponentContext {
    // === Mode ===
    /// Current mode name (e.g., "NORMAL", "INSERT", "VISUAL").
    pub mode: String,
    /// Mode subtype for visual modes (e.g., "CHAR", "LINE", "BLOCK").
    pub mode_subtype: Option<String>,

    // === Buffer ===
    /// Active buffer filename (basename only, e.g., "main.rs").
    pub filename: Option<String>,
    /// Full file path (e.g., "/home/user/project/src/main.rs").
    pub filepath: Option<String>,
    /// Buffer modified flag.
    pub modified: bool,
    /// Buffer readonly flag.
    pub readonly: bool,
    /// Filetype (e.g., "rust", "python", "markdown").
    pub filetype: Option<String>,

    // === Cursor ===
    /// Cursor line (1-indexed).
    pub line: usize,
    /// Cursor column (1-indexed, byte offset).
    pub column: usize,
    /// Total lines in buffer.
    pub total_lines: usize,

    // === Encoding ===
    /// File encoding (e.g., "utf-8", "latin1").
    pub encoding: String,
    /// Line ending style (e.g., "unix", "dos", "mac").
    pub line_ending: String,

    // === Window ===
    /// Terminal width.
    pub terminal_width: u16,
    /// Terminal height.
    pub terminal_height: u16,

    // === Extended (optional, provided by other modules) ===
    /// Git branch name (if git module provides).
    pub git_branch: Option<String>,
    /// Diagnostic counts (if LSP module provides).
    pub diagnostics: Option<DiagnosticCounts>,
}

/// Diagnostic counts from LSP or other sources.
#[derive(Debug, Clone, Default)]
pub struct DiagnosticCounts {
    /// Error count.
    pub errors: usize,
    /// Warning count.
    pub warnings: usize,
    /// Info count.
    pub info: usize,
    /// Hint count.
    pub hints: usize,
}

impl DiagnosticCounts {
    /// Check if there are any diagnostics.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.errors == 0 && self.warnings == 0 && self.info == 0 && self.hints == 0
    }

    /// Total count of all diagnostics.
    #[must_use]
    pub const fn total(&self) -> usize {
        self.errors + self.warnings + self.info + self.hints
    }
}

/// Output from a component render.
#[derive(Debug, Clone)]
pub struct ComponentOutput {
    /// Rendered text content.
    pub text: String,
    /// Optional style override (None = use section/theme default).
    pub style: Option<Style>,
    /// Whether this component should be displayed.
    pub visible: bool,
    /// Minimum width hint (for fixed-width components like mode).
    pub min_width: Option<usize>,
    /// Priority for truncation (higher = truncate later, 0-255).
    pub truncation_priority: u8,
}

impl ComponentOutput {
    /// Create a new visible component output.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: None,
            visible: true,
            min_width: None,
            truncation_priority: 128, // Default middle priority
        }
    }

    /// Create a hidden component output.
    #[must_use]
    pub const fn hidden() -> Self {
        Self {
            text: String::new(),
            style: None,
            visible: false,
            min_width: None,
            truncation_priority: 0,
        }
    }

    /// Set style override.
    #[must_use]
    pub const fn with_style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
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

impl Default for ComponentOutput {
    fn default() -> Self {
        Self::hidden()
    }
}

/// Trait for statusline components.
///
/// Components are pluggable units that render portions of the statusline.
/// Each component receives a snapshot of editor state and returns rendered text.
///
/// # Thread Safety
///
/// Components must be `Send + Sync`. The `render` method is called from the
/// main rendering thread. Do not perform blocking I/O in `render()` - use
/// cached state instead (see caching example below).
///
/// # Caching Example
///
/// ```ignore
/// use std::sync::RwLock;
///
/// struct BranchComponent {
///     cached_branch: RwLock<Option<String>>,
/// }
///
/// impl BranchComponent {
///     fn update_branch(&self, branch: Option<String>) {
///         *self.cached_branch.write().unwrap() = branch;
///     }
/// }
///
/// impl ComponentProvider for BranchComponent {
///     fn id(&self) -> &'static str { "branch" }
///
///     fn render(&self, _ctx: &ComponentContext) -> ComponentOutput {
///         match self.cached_branch.read().unwrap().as_ref() {
///             Some(branch) => ComponentOutput::new(format!(" {} ", branch)),
///             None => ComponentOutput::hidden(),
///         }
///     }
/// }
/// ```
pub trait ComponentProvider: Send + Sync {
    /// Unique component identifier (e.g., "mode", "filename", "branch").
    fn id(&self) -> &'static str;

    /// Render the component with current editor state.
    ///
    /// Should be fast - no I/O, no blocking.
    fn render(&self, ctx: &ComponentContext) -> ComponentOutput;

    /// Optional: Style override for specific mode.
    ///
    /// Called for mode-aware components (like section A) to get mode-specific colors.
    fn style_for_mode(&self, _mode: &str) -> Option<Style> {
        None
    }

    /// Optional: Whether this component needs frequent updates.
    ///
    /// - `false` (default): Only re-render on state change
    /// - `true`: Re-render every render cycle (for animated components)
    fn needs_frequent_update(&self) -> bool {
        false
    }
}

/// Key for component provider lookup in the registry.
///
/// Components are identified by their unique string ID (e.g., "mode", "branch", "diagnostics").
/// This wrapper enables type-safe registry lookups.
///
/// # Example
///
/// ```ignore
/// use reovim_driver_display::statusline::ComponentProviderKey;
///
/// // Register a custom component
/// let key = ComponentProviderKey::new("my-branch");
/// registry.register(key, Arc::new(MyBranchComponent));
///
/// // Lookup the component
/// let component = registry.get(&ComponentProviderKey::new("my-branch"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComponentProviderKey(String);

impl ComponentProviderKey {
    /// Create a new component key from a string ID.
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

impl ServiceKey for ComponentProviderKey {
    fn service_name() -> &'static str {
        "ComponentProvider"
    }
}

/// Registry for cross-module component providers.
///
/// Other modules can register custom statusline components during their `init()` phase.
/// The statusline provider can then look up these components by key.
///
/// # Cross-Module Registration
///
/// ```ignore
/// // In git module's init():
/// use reovim_driver_display::statusline::{
///     ComponentProviderKey, ComponentProviderRegistry,
/// };
///
/// fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
///     let registry = ctx.services.get_or_create::<ComponentProviderRegistry>();
///     registry.register(
///         ComponentProviderKey::new("branch"),
///         Arc::new(BranchComponent::new()),
///     );
///     ProbeResult::Success
/// }
/// ```
///
/// # Lookup in Statusline Provider
///
/// ```ignore
/// // In statusline provider's render():
/// let registry = ctx.services.get::<ComponentProviderRegistry>();
/// if let Some(registry) = registry {
///     if let Some(branch) = registry.get(&ComponentProviderKey::new("branch")) {
///         let output = branch.render(&component_ctx);
///         // Include in section...
///     }
/// }
/// ```
pub type ComponentProviderRegistry =
    MultiServiceRegistry<ComponentProviderKey, dyn ComponentProvider>;

#[cfg(test)]
mod tests {
    use super::*;

    struct TestComponent;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl ComponentProvider for TestComponent {
        fn id(&self) -> &'static str {
            "test"
        }

        fn render(&self, ctx: &ComponentContext) -> ComponentOutput {
            ComponentOutput::new(format!("Mode: {}", ctx.mode))
        }
    }

    #[test]
    fn test_component_context_default() {
        let ctx = ComponentContext::default();
        assert!(ctx.mode.is_empty());
        assert!(ctx.filename.is_none());
        assert_eq!(ctx.line, 0);
        assert_eq!(ctx.column, 0);
    }

    #[test]
    fn test_component_output_new() {
        let output = ComponentOutput::new("test");
        assert_eq!(output.text, "test");
        assert!(output.visible);
        assert!(output.style.is_none());
    }

    #[test]
    fn test_component_output_hidden() {
        let output = ComponentOutput::hidden();
        assert!(output.text.is_empty());
        assert!(!output.visible);
    }

    #[test]
    fn test_component_output_builder() {
        let output = ComponentOutput::new("test")
            .with_min_width(10)
            .with_priority(200)
            .when(true);

        assert_eq!(output.min_width, Some(10));
        assert_eq!(output.truncation_priority, 200);
        assert!(output.visible);
    }

    #[test]
    fn test_component_provider_render() {
        let component = TestComponent;
        let ctx = ComponentContext {
            mode: "NORMAL".to_string(),
            ..Default::default()
        };

        let output = component.render(&ctx);
        assert_eq!(output.text, "Mode: NORMAL");
    }

    #[test]
    fn test_diagnostic_counts() {
        let counts = DiagnosticCounts {
            errors: 2,
            warnings: 3,
            info: 1,
            hints: 0,
        };

        assert!(!counts.is_empty());
        assert_eq!(counts.total(), 6);

        let empty = DiagnosticCounts::default();
        assert!(empty.is_empty());
        assert_eq!(empty.total(), 0);
    }

    #[test]
    fn test_component_provider_key() {
        let key = ComponentProviderKey::new("branch");
        assert_eq!(key.id(), "branch");

        let key2 = ComponentProviderKey::new("branch".to_string());
        assert_eq!(key, key2);
    }

    #[test]
    fn test_component_provider_key_service_name() {
        assert_eq!(ComponentProviderKey::service_name(), "ComponentProvider");
    }

    #[test]
    fn test_component_provider_registry() {
        use std::sync::Arc;

        let registry = ComponentProviderRegistry::new();

        // Register a component
        let key = ComponentProviderKey::new("test");
        registry.register(key.clone(), Arc::new(TestComponent));

        // Lookup the component
        let component = registry.get(&key);
        assert!(component.is_some());

        let ctx = ComponentContext::default();
        let output = component.unwrap().render(&ctx);
        assert!(output.text.contains("Mode:"));
    }

    #[test]
    fn test_component_output_with_style() {
        let style = Style::new().fg(crate::Color::Red);
        let output = ComponentOutput::new("styled").with_style(style.clone());

        assert_eq!(output.text, "styled");
        assert!(output.visible);
        assert_eq!(output.style, Some(style));
    }

    #[test]
    fn test_component_output_default() {
        let output = ComponentOutput::default();
        assert!(output.text.is_empty());
        assert!(!output.visible);
        assert!(output.style.is_none());
        assert_eq!(output.truncation_priority, 0);
    }

    #[test]
    fn test_component_provider_style_for_mode_default() {
        let component = TestComponent;
        assert!(component.style_for_mode("NORMAL").is_none());
        assert!(component.style_for_mode("INSERT").is_none());
    }

    #[test]
    fn test_component_provider_needs_frequent_update_default() {
        let component = TestComponent;
        assert!(!component.needs_frequent_update());
    }

    /// A component that overrides the default trait methods.
    struct CustomComponent;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl ComponentProvider for CustomComponent {
        fn id(&self) -> &'static str {
            "custom"
        }

        fn render(&self, _ctx: &ComponentContext) -> ComponentOutput {
            ComponentOutput::new("custom")
        }

        fn style_for_mode(&self, mode: &str) -> Option<Style> {
            if mode == "INSERT" {
                Some(Style::new().fg(crate::Color::Green))
            } else {
                None
            }
        }

        fn needs_frequent_update(&self) -> bool {
            true
        }
    }

    #[test]
    fn test_custom_component_style_for_mode() {
        let component = CustomComponent;
        assert!(component.style_for_mode("INSERT").is_some());
        assert!(component.style_for_mode("NORMAL").is_none());
    }

    #[test]
    fn test_custom_component_needs_frequent_update() {
        let component = CustomComponent;
        assert!(component.needs_frequent_update());
    }
}
