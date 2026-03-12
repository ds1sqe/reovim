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
#[path = "component_tests.rs"]
mod tests;
