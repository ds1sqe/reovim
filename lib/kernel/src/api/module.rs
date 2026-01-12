//! Module identity, traits, and registration types.
//!
//! Linux equivalent: `include/linux/module.h`
//!
//! This module provides the core module mechanism for the kernel. Modules are
//! loadable components that can extend the editor's functionality. The kernel
//! defines the `Module` trait (mechanism); the runner implements loading (policy).
//!
//! # Linux Kernel Patterns
//!
//! The design follows Linux kernel patterns:
//! - `Module` trait ≈ `struct module` + `module_init`/`module_exit`
//! - `ProbeResult` ≈ `probe()` return with `-EPROBE_DEFER` support
//! - `RegistrationFlags` ≈ module flags and capabilities
//! - Registration types ≈ `struct platform_driver`, `struct notifier_block`

use std::{borrow::Cow, fmt};

use super::{
    context::ModuleContext,
    version::{API_VERSION, Version},
};

/// Unique identifier for a loadable module.
///
/// Convention: Use kebab-case names like "lang-rust", "feat-completion".
///
/// # Static vs Dynamic IDs
///
/// Module IDs can be either:
/// - **Static** (`&'static str`): For compile-time known modules, use `ModuleId::new()`
/// - **Dynamic** (`String`): For runtime-generated modules, use `ModuleId::from_string()`
///
/// Static IDs are preferred for performance (no allocation), but dynamic IDs
/// allow for user-defined or plugin-loaded modules with arbitrary names.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::ModuleId;
///
/// // Static ID (compile-time known)
/// let static_id = ModuleId::new("lang-rust");
///
/// // Dynamic ID (runtime generated)
/// let name = format!("user-plugin-{}", 42);
/// let dynamic_id = ModuleId::from_string(name);
///
/// // Both work the same way
/// assert_eq!(static_id.as_str(), "lang-rust");
/// assert_eq!(dynamic_id.as_str(), "user-plugin-42");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleId(Cow<'static, str>);

impl ModuleId {
    /// Create a new module identifier from a static string.
    ///
    /// This is the preferred way to create module IDs for statically-known modules.
    /// It's a const fn and involves no allocation.
    #[must_use]
    pub const fn new(id: &'static str) -> Self {
        Self(Cow::Borrowed(id))
    }

    /// Create a module identifier from an owned String.
    ///
    /// Use this for dynamically-generated module IDs (e.g., user plugins,
    /// runtime-loaded modules with user-provided names).
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // String operations aren't const-stable
    pub fn from_string(id: String) -> Self {
        Self(Cow::Owned(id))
    }

    /// Get the identifier string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Check if this is a static (borrowed) ID.
    #[must_use]
    pub const fn is_static(&self) -> bool {
        matches!(self.0, Cow::Borrowed(_))
    }

    /// Check if this is a dynamic (owned) ID.
    #[must_use]
    pub const fn is_dynamic(&self) -> bool {
        matches!(self.0, Cow::Owned(_))
    }
}

impl fmt::Display for ModuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&'static str> for ModuleId {
    fn from(s: &'static str) -> Self {
        Self::new(s)
    }
}

impl From<String> for ModuleId {
    fn from(s: String) -> Self {
        Self::from_string(s)
    }
}

/// Errors that can occur during module operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleError {
    /// Failed to load the shared object file.
    LoadFailed(String),

    /// Module does not export the required entry point.
    NoEntryPoint(String),

    /// Module initialization failed.
    InitFailed(String),

    /// Module API version is incompatible with kernel.
    IncompatibleVersion {
        /// Version the module requires.
        module: (u32, u32),
        /// Version the kernel provides.
        kernel: (u32, u32),
    },

    /// Module is in use by another module.
    InUse {
        /// The module that cannot be unloaded.
        module: ModuleId,
        /// The module that depends on it.
        by: ModuleId,
    },

    /// Module is not currently loaded.
    NotLoaded(ModuleId),

    /// Module file was not found.
    NotFound(String),
}

impl fmt::Display for ModuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LoadFailed(msg) => write!(f, "failed to load module: {msg}"),
            Self::NoEntryPoint(msg) => write!(f, "module missing entry point: {msg}"),
            Self::InitFailed(msg) => write!(f, "module initialization failed: {msg}"),
            Self::IncompatibleVersion { module, kernel } => {
                write!(
                    f,
                    "module requires API {}.{}, kernel provides {}.{}",
                    module.0, module.1, kernel.0, kernel.1
                )
            }
            Self::InUse { module, by } => {
                write!(f, "module '{module}' is in use by '{by}'")
            }
            Self::NotLoaded(id) => write!(f, "module '{id}' is not loaded"),
            Self::NotFound(name) => write!(f, "module '{name}' not found"),
        }
    }
}

impl std::error::Error for ModuleError {}

// ============================================================================
// Registration Flags (Linux-inspired)
// ============================================================================

/// Registration capability flags (Linux-inspired).
///
/// Like Linux kernel module flags, these control registration behavior.
/// Used by all registration types to indicate how the runner should handle
/// the registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)] // Flags struct intentionally uses multiple bools
pub struct RegistrationFlags {
    /// Registration is required (fail if cannot register).
    pub required: bool,
    /// Can be deferred if dependencies not ready (Linux: `-EPROBE_DEFER`).
    pub deferrable: bool,
    /// Should be registered early (before other modules).
    pub early: bool,
    /// Acts as fallback if no other handler matches.
    pub fallback: bool,
}

impl RegistrationFlags {
    /// Create default flags (all false).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            required: false,
            deferrable: false,
            early: false,
            fallback: false,
        }
    }

    /// Create flags for required registration.
    #[must_use]
    pub const fn required() -> Self {
        Self {
            required: true,
            deferrable: false,
            early: false,
            fallback: false,
        }
    }

    /// Create flags for deferrable registration.
    #[must_use]
    pub const fn deferrable() -> Self {
        Self {
            required: false,
            deferrable: true,
            early: false,
            fallback: false,
        }
    }

    // ========================================================================
    // Chainable Builders
    // ========================================================================

    /// Set the required flag (chainable).
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::api::v1::RegistrationFlags;
    ///
    /// let flags = RegistrationFlags::new().set_required().set_early();
    /// assert!(flags.is_required());
    /// assert!(flags.is_early());
    /// ```
    #[must_use]
    pub const fn set_required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Set the deferrable flag (chainable).
    #[must_use]
    pub const fn set_deferrable(mut self) -> Self {
        self.deferrable = true;
        self
    }

    /// Set the early flag (chainable).
    #[must_use]
    pub const fn set_early(mut self) -> Self {
        self.early = true;
        self
    }

    /// Set the fallback flag (chainable).
    #[must_use]
    pub const fn set_fallback(mut self) -> Self {
        self.fallback = true;
        self
    }

    // ========================================================================
    // Query Methods
    // ========================================================================

    /// Check if this registration is required.
    #[must_use]
    pub const fn is_required(&self) -> bool {
        self.required
    }

    /// Check if this registration is deferrable.
    #[must_use]
    pub const fn is_deferrable(&self) -> bool {
        self.deferrable
    }

    /// Check if this registration should happen early.
    #[must_use]
    pub const fn is_early(&self) -> bool {
        self.early
    }

    /// Check if this registration acts as a fallback.
    #[must_use]
    pub const fn is_fallback(&self) -> bool {
        self.fallback
    }
}

// ============================================================================
// Probe Result (Linux-inspired deferred probing)
// ============================================================================

/// Result of module probe/initialization.
///
/// Linux equivalent: Return value from `probe()` function.
/// - `Success` = success (Linux: `0`)
/// - `Defer` = try again later (Linux: `-EPROBE_DEFER`)
/// - `Failed` = permanent failure (Linux: negative errno)
///
/// # Example
///
/// ```ignore
/// fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
///     if !ctx.kernel.event_bus.has_subscriber("treesitter") {
///         return ProbeResult::Defer("waiting for treesitter".into());
///     }
///     ProbeResult::Success
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeResult {
    /// Probe succeeded, module is ready.
    Success,
    /// Probe deferred, retry later (with reason).
    /// Linux equivalent: `-EPROBE_DEFER`
    Defer(String),
    /// Probe failed permanently.
    Failed(ModuleError),
}

// ============================================================================
// Module Trait
// ============================================================================

/// Trait for all loadable modules.
///
/// Linux equivalent: `struct module` with `module_init`/`module_exit`.
///
/// Modules encapsulate functionality that can be loaded dynamically at runtime.
/// The kernel defines this trait (mechanism); the runner implements loading (policy).
///
/// # Lifecycle
///
/// 1. Module is loaded (by runner's module loader)
/// 2. `init()` is called with `ModuleContext`
///    - Returns `ProbeResult::Success` → module is ready
///    - Returns `ProbeResult::Defer` → retry later
///    - Returns `ProbeResult::Failed` → permanent failure
/// 3. Module is now `Running`
/// 4. `exit()` is called before unload
///
/// # Thread Safety
///
/// Modules must be `Send + Sync` as they may be accessed from multiple threads.
/// The threading model is:
///
/// - **Exclusive access methods** (`&mut self`): `init()` and `exit()` are guaranteed
///   to be called with exclusive access. The runner/kernel ensures no concurrent calls.
/// - **Shared access methods** (`&self`): `id()`, `name()`, `version()`, `commands()`,
///   `keybindings()`, `event_handlers()`, etc. may be called concurrently from multiple
///   threads. Implementations should return const data or use internal synchronization.
/// - **Hot reload methods**: `save_state()` (`&self`) and `restore_state()` (`&mut self`)
///   follow the same exclusive/shared access patterns.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::api::v1::*;
///
/// pub struct MyModule;
///
/// impl Module for MyModule {
///     fn id(&self) -> ModuleId { ModuleId::new("my-module") }
///     fn name(&self) -> &'static str { "My Module" }
///     fn version(&self) -> Version { Version::new(1, 0, 0) }
///
///     fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
///         pr_info!("MyModule initialized");
///         ProbeResult::Success
///     }
///
///     fn exit(&mut self) -> Result<(), ModuleError> {
///         pr_info!("MyModule exiting");
///         Ok(())
///     }
/// }
/// ```
pub trait Module: Send + Sync + 'static {
    // ========================================================================
    // Identity
    // ========================================================================

    /// Unique module identifier.
    ///
    /// Convention: kebab-case, e.g., "lang-rust", "feat-completion"
    fn id(&self) -> ModuleId;

    /// Human-readable name.
    fn name(&self) -> &'static str;

    /// Module version.
    fn version(&self) -> Version;

    /// Required kernel API version.
    ///
    /// Defaults to current API version. Override to require specific version.
    fn api_version(&self) -> Version {
        API_VERSION
    }

    // ========================================================================
    // Dependencies
    // ========================================================================

    /// Required dependencies (must be loaded before this module).
    ///
    /// The runner will ensure all required dependencies are loaded and initialized
    /// before calling `init()` on this module. If any dependency fails to load,
    /// this module will not be initialized.
    fn dependencies(&self) -> Vec<ModuleId> {
        Vec::new()
    }

    /// Optional dependencies (load before if available, but not required).
    ///
    /// # Semantics
    ///
    /// - Optional dependencies are loaded before this module **if they exist**
    /// - If an optional dependency fails to load, this module can still initialize
    /// - Unlike `dependencies()`, missing optional dependencies don't cause init failure
    /// - Useful for feature detection and graceful degradation
    ///
    /// # Use Cases
    ///
    /// - LSP enhancement: Load LSP plugin if available for better completions
    /// - Syntax integration: Use treesitter if available, fall back to regex
    /// - Theme support: Load icon theme if available
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn optional_dependencies(&self) -> Vec<ModuleId> {
    ///     vec![
    ///         ModuleId::new("feat-lsp"),       // Enhance with LSP if available
    ///         ModuleId::new("feat-treesitter"), // Better syntax if available
    ///     ]
    /// }
    /// ```
    fn optional_dependencies(&self) -> Vec<ModuleId> {
        Vec::new()
    }

    // ========================================================================
    // Lifecycle (Linux: module_init / module_exit)
    // ========================================================================

    /// Initialize module (Linux equivalent: `probe()` function).
    ///
    /// Returns `ProbeResult` to support deferred probing:
    /// - `Success` - Module is ready
    /// - `Defer(reason)` - Retry later (like Linux `-EPROBE_DEFER`)
    /// - `Failed(err)` - Permanent failure
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult;

    /// Cleanup module (Linux equivalent: `remove()` function).
    ///
    /// # Errors
    ///
    /// Returns `ModuleError` if cleanup fails.
    fn exit(&mut self) -> Result<(), ModuleError>;

    // ========================================================================
    // Registration (method-based per Clean Arch Proposal)
    // ========================================================================

    /// Get command registrations.
    fn commands(&self) -> Vec<CommandRegistration> {
        Vec::new()
    }

    /// Get keybinding registrations.
    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        Vec::new()
    }

    /// Get event handler registrations.
    fn event_handlers(&self) -> Vec<EventHandlerRegistration> {
        Vec::new()
    }

    // ========================================================================
    // Hot Reload Support
    // ========================================================================

    /// Whether this module supports hot reload.
    ///
    /// If true, `save_state()` and `restore_state()` should be implemented.
    /// Hot reload allows updating module code without restarting the editor.
    fn supports_hot_reload(&self) -> bool {
        false
    }

    /// Save module state for hot reload.
    ///
    /// Returns opaque bytes that `restore_state()` can use to restore state.
    /// Return `None` if no state to save.
    ///
    /// # Serialization Format
    ///
    /// The binary format is module-specific. Recommended practices:
    ///
    /// - **Include a version header** for forward compatibility (e.g., first 4 bytes)
    /// - **Use a stable serialization format** like `bincode`, `postcard`, or `rmp-serde`
    /// - **Keep state minimal** - only save what's necessary to restore user experience
    /// - **Handle missing fields gracefully** in `restore_state()`
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn save_state(&self) -> Option<Box<[u8]>> {
    ///     let state = MyModuleState {
    ///         version: 1,
    ///         cursor_history: self.cursor_history.clone(),
    ///         settings: self.settings.clone(),
    ///     };
    ///     bincode::serialize(&state).ok().map(Vec::into_boxed_slice)
    /// }
    /// ```
    fn save_state(&self) -> Option<Box<[u8]>> {
        None
    }

    /// Restore module state after hot reload.
    ///
    /// Called with bytes from previous `save_state()` call.
    ///
    /// # State Version Compatibility
    ///
    /// Modules should handle version mismatches gracefully:
    /// - If the saved state version is **newer** than current, return an error
    /// - If the saved state version is **older**, migrate or use defaults
    /// - If deserialization fails, return an error (module will re-initialize)
    ///
    /// # Errors
    ///
    /// Returns `ModuleError::InitFailed` if:
    /// - Hot reload is not supported
    /// - State version is incompatible
    /// - Deserialization fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn restore_state(&mut self, state: &[u8]) -> Result<(), ModuleError> {
    ///     let saved: MyModuleState = bincode::deserialize(state)
    ///         .map_err(|e| ModuleError::InitFailed(format!("deserialize: {e}")))?;
    ///
    ///     if saved.version > CURRENT_VERSION {
    ///         return Err(ModuleError::InitFailed("state version too new".into()));
    ///     }
    ///
    ///     self.cursor_history = saved.cursor_history;
    ///     self.settings = saved.settings;
    ///     Ok(())
    /// }
    /// ```
    fn restore_state(&mut self, _state: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::InitFailed("hot reload not supported".into()))
    }
}

// ============================================================================
// Module State
// ============================================================================

/// Module lifecycle state.
///
/// State machine:
/// ```text
/// Loaded -> Initializing -> Running -> Unloading -> (removed)
///    |           |            |           |
///    +---------->+--->--------+--->-------+---> Failed
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ModuleState {
    /// Module is loaded but not initialized.
    #[default]
    Loaded,
    /// Module is currently initializing.
    Initializing,
    /// Module is running normally.
    Running,
    /// Module is being unloaded.
    Unloading,
    /// Module failed (with error message).
    Failed(String),
}

impl ModuleState {
    /// Check if transition to another state is valid.
    #[must_use]
    pub const fn can_transition_to(&self, next: &Self) -> bool {
        // Any state can transition to Failed
        if matches!(next, Self::Failed(_)) {
            return true;
        }

        matches!(
            (self, next),
            (Self::Loaded, Self::Initializing)
                | (Self::Initializing, Self::Running)
                | (Self::Running, Self::Unloading)
        )
    }
}

impl fmt::Display for ModuleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Loaded => write!(f, "loaded"),
            Self::Initializing => write!(f, "initializing"),
            Self::Running => write!(f, "running"),
            Self::Unloading => write!(f, "unloading"),
            Self::Failed(msg) => write!(f, "failed: {msg}"),
        }
    }
}

// ============================================================================
// Module Info
// ============================================================================

/// Static module metadata.
///
/// Used for module discovery and display.
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    /// Unique identifier.
    pub id: ModuleId,
    /// Human-readable name.
    pub name: &'static str,
    /// Version.
    pub version: Version,
    /// Required API version.
    pub api_version: Version,
    /// Description.
    pub description: &'static str,
    /// Author(s).
    pub authors: &'static [&'static str],
    /// License.
    pub license: &'static str,
}

impl ModuleInfo {
    /// Create module info from a Module instance.
    #[must_use]
    pub fn from_module<M: Module>(module: &M) -> Self {
        Self {
            id: module.id(),
            name: module.name(),
            version: module.version(),
            api_version: module.api_version(),
            description: "",
            authors: &[],
            license: "",
        }
    }
}

// ============================================================================
// Registration Types (Linux kernel-inspired declarative metadata)
// ============================================================================

/// Command registration descriptor.
///
/// Linux equivalent: Like `struct file_operations` - declares command capabilities.
///
/// Fields align with `CommandTrait` in lib/core/src/command/traits.rs.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)] // Command capabilities use multiple bool flags
pub struct CommandRegistration {
    /// Unique command identifier (e.g., "delete", "yank", "motion:word").
    pub id: &'static str,
    /// Human-readable name for help display.
    pub name: &'static str,
    /// Description for help system.
    pub description: &'static str,
    /// Category for help grouping (e.g., "motion", "operator", "edit").
    pub category: Option<&'static str>,
    /// Whether command accepts a count prefix (e.g., 5j).
    pub accepts_count: bool,
    /// Whether command accepts a motion (e.g., dw, c$).
    pub accepts_motion: bool,
    /// Whether command is a "jump" (recorded in jump list).
    pub is_jump: bool,
    /// Whether command modifies buffer text (for undo grouping).
    pub is_text_modifying: bool,
    /// Dependencies on other commands (Linux: module dependencies).
    pub depends_on: &'static [&'static str],
    /// Registration flags.
    pub flags: RegistrationFlags,
}

impl CommandRegistration {
    /// Create a new command registration with required id.
    #[must_use]
    pub const fn new(id: &'static str) -> Self {
        Self {
            id,
            name: "",
            description: "",
            category: None,
            accepts_count: false,
            accepts_motion: false,
            is_jump: false,
            is_text_modifying: false,
            depends_on: &[],
            flags: RegistrationFlags::new(),
        }
    }

    /// Set the display name.
    #[must_use]
    pub const fn with_name(mut self, name: &'static str) -> Self {
        self.name = name;
        self
    }

    /// Set the description.
    #[must_use]
    pub const fn with_description(mut self, desc: &'static str) -> Self {
        self.description = desc;
        self
    }

    /// Set the category.
    #[must_use]
    pub const fn with_category(mut self, cat: &'static str) -> Self {
        self.category = Some(cat);
        self
    }

    /// Mark command as accepting a count.
    #[must_use]
    pub const fn with_count(mut self) -> Self {
        self.accepts_count = true;
        self
    }

    /// Mark command as accepting a motion.
    #[must_use]
    pub const fn with_motion(mut self) -> Self {
        self.accepts_motion = true;
        self
    }

    /// Mark command as a jump.
    #[must_use]
    pub const fn with_jump(mut self) -> Self {
        self.is_jump = true;
        self
    }

    /// Mark command as text-modifying.
    #[must_use]
    pub const fn with_text_modifying(mut self) -> Self {
        self.is_text_modifying = true;
        self
    }

    /// Set dependencies.
    #[must_use]
    pub const fn with_depends_on(mut self, deps: &'static [&'static str]) -> Self {
        self.depends_on = deps;
        self
    }

    /// Set registration flags.
    #[must_use]
    pub const fn with_flags(mut self, flags: RegistrationFlags) -> Self {
        self.flags = flags;
        self
    }
}

/// Keybinding registration descriptor.
///
/// Linux equivalent: Like `struct input_device_id` - declares key matching.
#[derive(Debug, Clone)]
pub struct KeybindingRegistration {
    /// Key sequence in vim notation (e.g., `"dd"`, `"<C-w>h"`, `"<Space>ff"`).
    pub keys: &'static str,
    /// Command ID to invoke.
    pub command_id: &'static str,
    /// Modes where binding is active (e.g., `&["normal"]`, `&["normal", "visual"]`).
    /// Empty slice means all modes (like Linux's match-all).
    pub modes: &'static [&'static str],
    /// Description for which-key / help.
    pub description: &'static str,
    /// Category for which-key grouping (e.g., "window", "file", "search").
    pub category: Option<&'static str>,
    /// Whether binding is enabled (for conditional keybindings).
    pub enabled: bool,
    /// Priority for conflict resolution (lower = higher priority).
    /// Like Linux driver priority for matching.
    pub priority: u32,
    /// Dependencies on other keybindings or commands.
    pub depends_on: &'static [&'static str],
    /// Registration flags.
    pub flags: RegistrationFlags,
}

impl KeybindingRegistration {
    /// Create a new keybinding registration.
    #[must_use]
    pub const fn new(keys: &'static str, command_id: &'static str) -> Self {
        Self {
            keys,
            command_id,
            modes: &[],
            description: "",
            category: None,
            enabled: true,
            priority: 100, // Default plugin priority
            depends_on: &[],
            flags: RegistrationFlags::new(),
        }
    }

    /// Set active modes.
    #[must_use]
    pub const fn with_modes(mut self, modes: &'static [&'static str]) -> Self {
        self.modes = modes;
        self
    }

    /// Set description.
    #[must_use]
    pub const fn with_description(mut self, desc: &'static str) -> Self {
        self.description = desc;
        self
    }

    /// Set category.
    #[must_use]
    pub const fn with_category(mut self, cat: &'static str) -> Self {
        self.category = Some(cat);
        self
    }

    /// Disable the keybinding.
    #[must_use]
    pub const fn with_disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Set priority.
    #[must_use]
    pub const fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Set dependencies.
    #[must_use]
    pub const fn with_depends_on(mut self, deps: &'static [&'static str]) -> Self {
        self.depends_on = deps;
        self
    }

    /// Set registration flags.
    #[must_use]
    pub const fn with_flags(mut self, flags: RegistrationFlags) -> Self {
        self.flags = flags;
        self
    }
}

/// Event handler registration descriptor.
///
/// Linux equivalent: Like `struct notifier_block` - declares event subscription.
///
/// Priority convention (from EventBus):
/// - 0-50: Core handlers (kernel-level)
/// - 100: Default plugin priority
/// - 200+: Cleanup/late handlers
#[derive(Debug, Clone)]
pub struct EventHandlerRegistration {
    /// Event type name (e.g., `BufferChanged`, `CursorMoved`).
    pub event_type: &'static str,
    /// Handler priority (lower = called earlier).
    pub priority: u32,
    /// Description for debugging/introspection.
    pub description: &'static str,
    /// Whether handler auto-unsubscribes after one event (one-shot).
    pub once: bool,
    /// Optional target component ID for scoped events.
    pub target_component: Option<&'static str>,
    /// Dependencies on other handlers or modules.
    pub depends_on: &'static [&'static str],
    /// Registration flags.
    pub flags: RegistrationFlags,
}

impl EventHandlerRegistration {
    /// Create a new event handler registration.
    #[must_use]
    pub const fn new(event_type: &'static str) -> Self {
        Self {
            event_type,
            priority: 100, // Default plugin priority
            description: "",
            once: false,
            target_component: None,
            depends_on: &[],
            flags: RegistrationFlags::new(),
        }
    }

    /// Set priority.
    #[must_use]
    pub const fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Set description.
    #[must_use]
    pub const fn with_description(mut self, desc: &'static str) -> Self {
        self.description = desc;
        self
    }

    /// Mark as one-shot handler.
    #[must_use]
    pub const fn with_once(mut self) -> Self {
        self.once = true;
        self
    }

    /// Set target component.
    #[must_use]
    pub const fn with_target(mut self, component: &'static str) -> Self {
        self.target_component = Some(component);
        self
    }

    /// Set dependencies.
    #[must_use]
    pub const fn with_depends_on(mut self, deps: &'static [&'static str]) -> Self {
        self.depends_on = deps;
        self
    }

    /// Set registration flags.
    #[must_use]
    pub const fn with_flags(mut self, flags: RegistrationFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set core priority (clamped to 0-50 range).
    #[must_use]
    pub const fn core_priority(mut self, priority: u32) -> Self {
        self.priority = if priority > 50 { 50 } else { priority };
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // ModuleId tests
    // ========================================================================

    #[test]
    fn test_module_id() {
        let id = ModuleId::new("lang-rust");
        assert_eq!(id.as_str(), "lang-rust");
        assert_eq!(format!("{id}"), "lang-rust");
    }

    #[test]
    fn test_module_id_equality() {
        let id1 = ModuleId::new("lang-rust");
        let id2 = ModuleId::new("lang-rust");
        let id3 = ModuleId::new("lang-python");
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_module_id_dynamic() {
        let name = format!("user-plugin-{}", 42);
        let id = ModuleId::from_string(name);
        assert_eq!(id.as_str(), "user-plugin-42");
        assert!(id.is_dynamic());
        assert!(!id.is_static());
    }

    #[test]
    fn test_module_id_static() {
        let id = ModuleId::new("lang-rust");
        assert!(id.is_static());
        assert!(!id.is_dynamic());
    }

    #[test]
    fn test_module_id_static_dynamic_equality() {
        // Static and dynamic IDs with same content should be equal
        let static_id = ModuleId::new("test-module");
        let dynamic_id = ModuleId::from_string("test-module".to_string());

        assert_eq!(static_id, dynamic_id);
        assert_eq!(static_id.as_str(), dynamic_id.as_str());
    }

    #[test]
    fn test_module_id_from_traits() {
        // Test From<&'static str>
        let id1: ModuleId = "lang-rust".into();
        assert_eq!(id1.as_str(), "lang-rust");

        // Test From<String>
        let id2: ModuleId = String::from("lang-python").into();
        assert_eq!(id2.as_str(), "lang-python");
    }

    // ========================================================================
    // ModuleError tests
    // ========================================================================

    #[test]
    fn test_module_error_display() {
        let err = ModuleError::NotFound("test-module".into());
        assert_eq!(format!("{err}"), "module 'test-module' not found");

        let err = ModuleError::IncompatibleVersion {
            module: (2, 0),
            kernel: (1, 5),
        };
        assert!(format!("{err}").contains("2.0"));
        assert!(format!("{err}").contains("1.5"));
    }

    #[test]
    fn test_module_error_variants() {
        let err = ModuleError::LoadFailed("dlopen failed".into());
        assert!(format!("{err}").contains("load"));

        let err = ModuleError::NoEntryPoint("reovim_module".into());
        assert!(format!("{err}").contains("entry point"));

        let err = ModuleError::InitFailed("config missing".into());
        assert!(format!("{err}").contains("initialization failed"));

        let err = ModuleError::InUse {
            module: ModuleId::new("core"),
            by: ModuleId::new("lang-rust"),
        };
        assert!(format!("{err}").contains("in use"));

        let err = ModuleError::NotLoaded(ModuleId::new("missing"));
        assert!(format!("{err}").contains("not loaded"));
    }

    // ========================================================================
    // RegistrationFlags tests
    // ========================================================================

    #[test]
    fn test_registration_flags_default() {
        let flags = RegistrationFlags::new();
        assert!(!flags.required);
        assert!(!flags.deferrable);
        assert!(!flags.early);
        assert!(!flags.fallback);

        // Default trait should match new()
        let default_flags = RegistrationFlags::default();
        assert_eq!(flags, default_flags);
    }

    #[test]
    fn test_registration_flags_required() {
        let flags = RegistrationFlags::required();
        assert!(flags.required);
        assert!(!flags.deferrable);
        assert!(!flags.early);
        assert!(!flags.fallback);
    }

    #[test]
    fn test_registration_flags_deferrable() {
        let flags = RegistrationFlags::deferrable();
        assert!(!flags.required);
        assert!(flags.deferrable);
        assert!(!flags.early);
        assert!(!flags.fallback);
    }

    #[test]
    fn test_registration_flags_chainable_builders() {
        // Test chaining multiple flags
        let flags = RegistrationFlags::new().set_required().set_early();

        assert!(flags.required);
        assert!(!flags.deferrable);
        assert!(flags.early);
        assert!(!flags.fallback);

        // Test all flags together
        let all_flags = RegistrationFlags::new()
            .set_required()
            .set_deferrable()
            .set_early()
            .set_fallback();

        assert!(all_flags.required);
        assert!(all_flags.deferrable);
        assert!(all_flags.early);
        assert!(all_flags.fallback);
    }

    #[test]
    fn test_registration_flags_query_methods() {
        let flags = RegistrationFlags::new().set_required().set_early();

        assert!(flags.is_required());
        assert!(!flags.is_deferrable());
        assert!(flags.is_early());
        assert!(!flags.is_fallback());
    }

    // ========================================================================
    // ProbeResult tests
    // ========================================================================

    #[test]
    fn test_probe_result_success() {
        let result = ProbeResult::Success;
        assert_eq!(result, ProbeResult::Success);
    }

    #[test]
    fn test_probe_result_defer() {
        let result = ProbeResult::Defer("waiting for treesitter".into());
        if let ProbeResult::Defer(reason) = &result {
            assert!(reason.contains("treesitter"));
        } else {
            panic!("expected Defer variant");
        }
    }

    #[test]
    fn test_probe_result_failed() {
        let err = ModuleError::InitFailed("config missing".into());
        let result = ProbeResult::Failed(err.clone());
        if let ProbeResult::Failed(e) = result {
            assert_eq!(e, err);
        } else {
            panic!("expected Failed variant");
        }
    }

    // ========================================================================
    // ModuleState tests
    // ========================================================================

    #[test]
    fn test_module_state_default() {
        let state = ModuleState::default();
        assert_eq!(state, ModuleState::Loaded);
    }

    #[test]
    fn test_module_state_display() {
        assert_eq!(format!("{}", ModuleState::Loaded), "loaded");
        assert_eq!(format!("{}", ModuleState::Initializing), "initializing");
        assert_eq!(format!("{}", ModuleState::Running), "running");
        assert_eq!(format!("{}", ModuleState::Unloading), "unloading");
        assert_eq!(format!("{}", ModuleState::Failed("error".into())), "failed: error");
    }

    #[test]
    fn test_module_state_valid_transitions() {
        // Loaded -> Initializing
        assert!(ModuleState::Loaded.can_transition_to(&ModuleState::Initializing));

        // Initializing -> Running
        assert!(ModuleState::Initializing.can_transition_to(&ModuleState::Running));

        // Running -> Unloading
        assert!(ModuleState::Running.can_transition_to(&ModuleState::Unloading));

        // Any state -> Failed
        assert!(ModuleState::Loaded.can_transition_to(&ModuleState::Failed("err".into())));
        assert!(ModuleState::Initializing.can_transition_to(&ModuleState::Failed("err".into())));
        assert!(ModuleState::Running.can_transition_to(&ModuleState::Failed("err".into())));
        assert!(ModuleState::Unloading.can_transition_to(&ModuleState::Failed("err".into())));
    }

    #[test]
    fn test_module_state_invalid_transitions() {
        // Can't skip states
        assert!(!ModuleState::Loaded.can_transition_to(&ModuleState::Running));
        assert!(!ModuleState::Loaded.can_transition_to(&ModuleState::Unloading));

        // Can't go backwards
        assert!(!ModuleState::Running.can_transition_to(&ModuleState::Initializing));
        assert!(!ModuleState::Running.can_transition_to(&ModuleState::Loaded));
        assert!(!ModuleState::Initializing.can_transition_to(&ModuleState::Loaded));
    }

    // ========================================================================
    // CommandRegistration tests
    // ========================================================================

    #[test]
    fn test_command_registration_new() {
        let reg = CommandRegistration::new("delete");
        assert_eq!(reg.id, "delete");
        assert_eq!(reg.name, "");
        assert_eq!(reg.description, "");
        assert!(reg.category.is_none());
        assert!(!reg.accepts_count);
        assert!(!reg.accepts_motion);
        assert!(!reg.is_jump);
        assert!(!reg.is_text_modifying);
        assert!(reg.depends_on.is_empty());
    }

    #[test]
    fn test_command_registration_builder() {
        let reg = CommandRegistration::new("delete")
            .with_name("Delete")
            .with_description("Delete text")
            .with_category("operator")
            .with_count()
            .with_motion()
            .with_text_modifying()
            .with_depends_on(&["yank"])
            .with_flags(RegistrationFlags::required());

        assert_eq!(reg.id, "delete");
        assert_eq!(reg.name, "Delete");
        assert_eq!(reg.description, "Delete text");
        assert_eq!(reg.category, Some("operator"));
        assert!(reg.accepts_count);
        assert!(reg.accepts_motion);
        assert!(!reg.is_jump);
        assert!(reg.is_text_modifying);
        assert_eq!(reg.depends_on, &["yank"]);
        assert!(reg.flags.required);
    }

    #[test]
    fn test_command_registration_jump() {
        let reg = CommandRegistration::new("goto-definition").with_jump();
        assert!(reg.is_jump);
    }

    // ========================================================================
    // KeybindingRegistration tests
    // ========================================================================

    #[test]
    fn test_keybinding_registration_new() {
        let reg = KeybindingRegistration::new("dd", "delete-line");
        assert_eq!(reg.keys, "dd");
        assert_eq!(reg.command_id, "delete-line");
        assert!(reg.modes.is_empty()); // All modes
        assert_eq!(reg.description, "");
        assert!(reg.category.is_none());
        assert!(reg.enabled);
        assert_eq!(reg.priority, 100); // Default plugin priority
        assert!(reg.depends_on.is_empty());
    }

    #[test]
    fn test_keybinding_registration_builder() {
        let reg = KeybindingRegistration::new("<C-w>h", "window-left")
            .with_modes(&["normal"])
            .with_description("Move to left window")
            .with_category("window")
            .with_priority(50)
            .with_depends_on(&["window-split"])
            .with_flags(RegistrationFlags::deferrable());

        assert_eq!(reg.keys, "<C-w>h");
        assert_eq!(reg.command_id, "window-left");
        assert_eq!(reg.modes, &["normal"]);
        assert_eq!(reg.description, "Move to left window");
        assert_eq!(reg.category, Some("window"));
        assert!(reg.enabled);
        assert_eq!(reg.priority, 50);
        assert_eq!(reg.depends_on, &["window-split"]);
        assert!(reg.flags.deferrable);
    }

    #[test]
    fn test_keybinding_registration_disabled() {
        let reg = KeybindingRegistration::new("gd", "goto-definition").with_disabled();
        assert!(!reg.enabled);
    }

    // ========================================================================
    // EventHandlerRegistration tests
    // ========================================================================

    #[test]
    fn test_event_handler_registration_new() {
        let reg = EventHandlerRegistration::new("BufferChanged");
        assert_eq!(reg.event_type, "BufferChanged");
        assert_eq!(reg.priority, 100); // Default plugin priority
        assert_eq!(reg.description, "");
        assert!(!reg.once);
        assert!(reg.target_component.is_none());
        assert!(reg.depends_on.is_empty());
    }

    #[test]
    fn test_event_handler_registration_builder() {
        let reg = EventHandlerRegistration::new("CursorMoved")
            .with_priority(50)
            .with_description("Update cursor highlight")
            .with_target("treesitter")
            .with_depends_on(&["syntax-highlight"])
            .with_flags(RegistrationFlags::deferrable());

        assert_eq!(reg.event_type, "CursorMoved");
        assert_eq!(reg.priority, 50);
        assert_eq!(reg.description, "Update cursor highlight");
        assert!(!reg.once);
        assert_eq!(reg.target_component, Some("treesitter"));
        assert_eq!(reg.depends_on, &["syntax-highlight"]);
        assert!(reg.flags.deferrable);
    }

    #[test]
    fn test_event_handler_registration_once() {
        let reg = EventHandlerRegistration::new("ModuleLoaded").with_once();
        assert!(reg.once);
    }

    #[test]
    fn test_event_handler_core_priority() {
        // Core priority clamped to 0-50
        let reg = EventHandlerRegistration::new("BufferChanged").core_priority(25);
        assert_eq!(reg.priority, 25);

        let reg = EventHandlerRegistration::new("BufferChanged").core_priority(100);
        assert_eq!(reg.priority, 50); // Clamped

        let reg = EventHandlerRegistration::new("BufferChanged").core_priority(0);
        assert_eq!(reg.priority, 0);
    }

    // ========================================================================
    // ModuleInfo tests
    // ========================================================================

    #[test]
    fn test_module_info_fields() {
        let info = ModuleInfo {
            id: ModuleId::new("lang-rust"),
            name: "Rust Language Support",
            version: Version::new(1, 2, 3),
            api_version: API_VERSION,
            description: "Rust syntax and LSP",
            authors: &["Author 1", "Author 2"],
            license: "MIT",
        };

        assert_eq!(info.id.as_str(), "lang-rust");
        assert_eq!(info.name, "Rust Language Support");
        assert_eq!(info.version.major, 1);
        assert_eq!(info.version.minor, 2);
        assert_eq!(info.version.patch, 3);
        assert_eq!(info.description, "Rust syntax and LSP");
        assert_eq!(info.authors.len(), 2);
        assert_eq!(info.license, "MIT");
    }

    // ========================================================================
    // Module trait tests (from plan)
    // ========================================================================

    /// Verify Module trait is object-safe (can be used as `dyn Module`).
    /// This is critical for dynamic module loading via libloading.
    #[test]
    fn test_module_trait_object_safety() {
        // These function signatures compile = trait is object safe
        fn _accepts_module_ref(_: &dyn Module) {}
        fn _accepts_boxed_module(_: Box<dyn Module>) {}

        // Trait is object safe if this compiles
    }

    #[test]
    fn test_module_info_from_module() {
        // Create a test module implementation
        struct TestModule;

        impl Module for TestModule {
            fn id(&self) -> ModuleId {
                ModuleId::new("test-module")
            }
            fn name(&self) -> &'static str {
                "Test Module"
            }
            fn version(&self) -> Version {
                Version::new(2, 1, 0)
            }
            fn api_version(&self) -> Version {
                Version::new(1, 0, 0)
            }
            fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
                ProbeResult::Success
            }
            fn exit(&mut self) -> Result<(), ModuleError> {
                Ok(())
            }
        }

        let module = TestModule;
        let info = ModuleInfo::from_module(&module);

        assert_eq!(info.id, ModuleId::new("test-module"));
        assert_eq!(info.name, "Test Module");
        assert_eq!(info.version, Version::new(2, 1, 0));
        assert_eq!(info.api_version, Version::new(1, 0, 0));
    }

    #[test]
    fn test_module_deferred_probing() {
        // Test module that uses deferred probing pattern
        struct DeferredModule {
            ready: bool,
        }

        impl Module for DeferredModule {
            fn id(&self) -> ModuleId {
                ModuleId::new("deferred")
            }
            fn name(&self) -> &'static str {
                "Deferred Module"
            }
            fn version(&self) -> Version {
                Version::new(1, 0, 0)
            }
            fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
                if self.ready {
                    ProbeResult::Success
                } else {
                    ProbeResult::Defer("waiting for dependency".into())
                }
            }
            fn exit(&mut self) -> Result<(), ModuleError> {
                Ok(())
            }
        }

        // Verify module implements the trait correctly with both states
        let ready_module = DeferredModule { ready: true };
        assert_eq!(ready_module.id(), ModuleId::new("deferred"));
        assert_eq!(ready_module.name(), "Deferred Module");

        let not_ready = DeferredModule { ready: false };
        assert_eq!(not_ready.id(), ModuleId::new("deferred"));
        // Both ready and not-ready modules have the same identity
    }

    #[test]
    fn test_module_state_edge_cases() {
        // Test Failed state transitions
        let failed = ModuleState::Failed("error".into());

        // Failed can only transition to Failed (another error)
        assert!(!failed.can_transition_to(&ModuleState::Loaded));
        assert!(!failed.can_transition_to(&ModuleState::Initializing));
        assert!(!failed.can_transition_to(&ModuleState::Running));
        assert!(!failed.can_transition_to(&ModuleState::Unloading));
        assert!(failed.can_transition_to(&ModuleState::Failed("new error".into())));

        // Test Unloading state transitions
        let unloading = ModuleState::Unloading;

        // Unloading should only allow transition to Failed
        assert!(!unloading.can_transition_to(&ModuleState::Loaded));
        assert!(!unloading.can_transition_to(&ModuleState::Initializing));
        assert!(!unloading.can_transition_to(&ModuleState::Running));
        assert!(unloading.can_transition_to(&ModuleState::Failed("error".into())));
    }
}
