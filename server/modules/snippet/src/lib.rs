//! Snippet expansion module for reovim (#136).
//!
//! Provides TextMate/LSP-compatible snippet expansion with intelligent
//! tab stop navigation, placeholder mirroring, variable resolution,
//! regex transforms, and choice support.
//!
//! # Architecture
//!
//! - `ast` - Snippet element types (full EBNF grammar coverage)
//! - `parser` - Recursive descent parser (full TextMate/LSP grammar)
//! - `engine` - Active snippet tracking with position updates
//! - `transform` - Regex transform execution with case modifiers
//! - `variables` - Built-in variable resolver (`TM_FILENAME`, etc.)
//! - `provider` - Snippet provider trait and registry
//! - `loader` - JSON file loader (VSCode-compatible format)
//! - `resolver` - Mode key resolver for snippet navigation mode
//! - `state` - Per-client session extension state
//! - `command` - Command handlers (expand, jump next/prev, cancel)
//!
//! # Commands
//!
//! - `snippet:expand` (`<C-s>` in insert mode) - Expand snippet at cursor
//! - `snippet:jump-next` (`<Tab>` in snippet mode) - Next tab stop
//! - `snippet:jump-prev` (`<S-Tab>` in snippet mode) - Previous tab stop
//! - `snippet:cancel` (`<Esc>` in snippet mode) - Cancel snippet navigation

pub mod ast;
pub mod command;
pub mod config;
pub mod engine;
mod expander;
pub mod ids;
pub mod inheritance;
pub mod loader;
pub mod loader_friendly;
pub mod parser;
pub mod project;
pub mod provider;
pub mod resolver;
pub mod state;
pub mod transform;
pub mod variables;

pub use config::SnippetParentMode;

use std::path::Path;

use std::sync::Arc;

use {
    reovim_driver_command::{CommandHandler, CommandHandlerStore, CommandProvider},
    reovim_driver_input::{KeybindingStore, ModeInfo, ModeInfoStore, ResolverRegistry},
    reovim_driver_session::SnippetExpanderRegistry,
    reovim_kernel::api::v1::{
        CursorStyle, KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId,
        ProbeResult, Version, pr_info,
    },
};

use crate::{
    loader::JsonSnippetProvider,
    loader_friendly::FriendlySnippetsProvider,
    provider::{SnippetRegistry, SnippetRegistryHandle},
};

/// Build a snippet registry from the module data directory.
///
/// Loads providers in priority order (first registered = highest priority):
/// 1. User snippets: `{data_dir}/user/` (or legacy `{data_dir}/snippets/`)
/// 2. Friendly-snippets: `{data_dir}/friendly-snippets/` (if present)
/// 3. Built-in snippets: `{data_dir}/built-in/`
#[must_use]
pub fn build_registry(data_dir: &Path) -> SnippetRegistry {
    let mut registry = SnippetRegistry::new();

    // 1. User snippets (highest priority)
    let user_dir = data_dir.join("user");
    let legacy_dir = data_dir.join("snippets");

    let user_source = if user_dir.exists() {
        &user_dir
    } else {
        // Backward compat: treat legacy `snippets/` as user source
        &legacy_dir
    };

    if let Ok(provider) = JsonSnippetProvider::load_directory(user_source) {
        registry.register(Box::new(provider));
    }

    // 2. Friendly-snippets (middle priority)
    let friendly_dir = data_dir.join("friendly-snippets");
    if friendly_dir.join("package.json").exists()
        && let Ok(provider) = FriendlySnippetsProvider::load(&friendly_dir)
    {
        registry.register(Box::new(provider));
    }

    // 3. Built-in snippets (lowest priority)
    let builtin_dir = data_dir.join("built-in");
    if let Ok(provider) = JsonSnippetProvider::load_directory(&builtin_dir) {
        registry.register(Box::new(provider));
    }

    registry
}

/// Snippet expansion module.
///
/// Manages snippet loading, parsing, expansion, and tab stop navigation.
/// Registers commands, mode resolver, and keybindings during init.
pub struct SnippetModule {
    /// Snippet registry handle (kept alive for command handler access and hot reload).
    handle: Option<SnippetRegistryHandle>,
    /// Data directory for reload support.
    data_dir: Option<std::path::PathBuf>,
}

impl SnippetModule {
    /// Create a new snippet module.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            handle: None,
            data_dir: None,
        }
    }
}

impl Default for SnippetModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for SnippetModule {
    fn id(&self) -> ModuleId {
        ids::MODULE
    }

    fn name(&self) -> &'static str {
        "Snippet"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // 1. Load snippet files from data directory (multi-source hierarchy)
        let handle = SnippetRegistryHandle::new(build_registry(&ctx.data_dir));
        self.handle = Some(handle.clone());
        self.data_dir = Some(ctx.data_dir.clone());

        // Read parent mode from adapter-injected config (e.g., vim-snippet)
        let parent_insert = ctx
            .services
            .get::<SnippetParentMode>()
            .expect("SnippetParentMode must be registered (by adapter) before snippet")
            .mode()
            .clone();
        let modes = ctx.services.get_or_create::<ModeInfoStore>();

        // 2. Register command handlers with return mode
        let store = ctx.services.get_or_create::<CommandHandlerStore>();
        let commands = command::all_commands(handle, parent_insert.clone(), ctx.data_dir.clone());
        let command_count = commands.len();
        for cmd_handler in commands {
            store.add(cmd_handler);
        }

        // 3. Register snippet resolver with looked-up parent
        let resolvers = ctx.services.get_or_create::<ResolverRegistry>();
        resolvers.register(resolver::SnippetResolver::with_parent(parent_insert.clone()));

        // 4. Register mode info for display
        modes.add(ModeInfo {
            id: ids::NAVIGATING_MODE,
            display_name: "SNIPPET",
            cursor_style: CursorStyle::Bar,
            accepts_char_input: true,
            has_selection: true,
            inherits_from: Some(parent_insert),
            is_entry: false,
        });

        // 5. Register keybindings
        let keybinding_store = ctx.services.get_or_create::<KeybindingStore>();
        keybinding_store.add_all(self.keybindings());

        // 6. Register SnippetExpander implementation (#542: decouple completion from this module).
        let expander_registry = ctx.services.get_or_create::<SnippetExpanderRegistry>();
        expander_registry.register(Arc::new(expander::SnippetExpanderImpl));

        pr_info!("Snippet module initialized with {command_count} commands");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        self.handle = None;
        self.data_dir = None;
        Ok(())
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        vec![
            // Snippet navigating mode: tab stop navigation
            KeybindingRegistration::new("<Tab>", ids::JUMP_NEXT)
                .with_modes(&["snippet:navigating"])
                .with_description("Jump to next tab stop")
                .with_category("snippet"),
            // S-Tab navigates to previous tab stop
            KeybindingRegistration::new("<S-Tab>", ids::JUMP_PREV)
                .with_modes(&["snippet:navigating"])
                .with_description("Jump to previous tab stop")
                .with_category("snippet"),
            // Esc cancels snippet navigation
            KeybindingRegistration::new("<Esc>", ids::CANCEL)
                .with_modes(&["snippet:navigating"])
                .with_description("Cancel snippet navigation")
                .with_category("snippet"),
        ]
    }
}

impl CommandProvider for SnippetModule {
    fn command_handlers(&self) -> Vec<Box<dyn CommandHandler>> {
        let handle = self
            .handle
            .clone()
            .unwrap_or_else(|| SnippetRegistryHandle::new(SnippetRegistry::new()));
        // Fallback ModeId for CommandProvider (testing/FFI only).
        // In production, init() resolves the real mode from ModeInfoStore.
        let fallback_mode = reovim_kernel::api::v1::ModeId::new(ModuleId::new("editor"), "insert");
        let data_dir = self
            .data_dir
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp/reovim-snippet-fallback"));
        command::all_commands(handle, fallback_mode, data_dir)
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(SnippetModule);

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    // =========================================================================
    // SnippetModule construction
    // =========================================================================

    #[test]
    fn test_new_creates_module() {
        let module = SnippetModule::new();
        assert!(module.handle.is_none());
    }

    #[test]
    fn test_default_creates_same_as_new() {
        let from_new = SnippetModule::new();
        let from_default = SnippetModule::default();
        assert!(from_new.handle.is_none());
        assert!(from_default.handle.is_none());
    }

    // =========================================================================
    // Module trait
    // =========================================================================

    #[test]
    fn test_module_id() {
        let module = SnippetModule::new();
        assert_eq!(module.id().as_str(), "snippet");
    }

    #[test]
    fn test_module_name() {
        let module = SnippetModule::new();
        assert_eq!(module.name(), "Snippet");
    }

    #[test]
    fn test_module_version() {
        let module = SnippetModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 1);
        assert_eq!(version.patch, 0);
    }

    // =========================================================================
    // Exit
    // =========================================================================

    #[test]
    fn test_exit_clears_handle() {
        let mut module = SnippetModule::new();
        module.handle = Some(SnippetRegistryHandle::new(SnippetRegistry::new()));
        assert!(module.exit().is_ok());
        assert!(module.handle.is_none());
        assert!(module.data_dir.is_none());
    }

    #[test]
    fn test_exit_ok_when_no_handle() {
        let mut module = SnippetModule::new();
        assert!(module.exit().is_ok());
    }

    // =========================================================================
    // Keybindings
    // =========================================================================

    #[test]
    fn test_keybindings_count() {
        let module = SnippetModule::new();
        // Only snippet:navigating bindings remain (Tab, S-Tab, Esc)
        assert_eq!(module.keybindings().len(), 3);
    }

    #[test]
    fn test_keybindings_jump_next() {
        let module = SnippetModule::new();
        let bindings = module.keybindings();
        let next = &bindings[0];
        assert_eq!(next.keys, "<Tab>");
        assert_eq!(next.command_id, ids::JUMP_NEXT);
    }

    #[test]
    fn test_keybindings_jump_prev() {
        let module = SnippetModule::new();
        let bindings = module.keybindings();
        let prev = &bindings[1];
        assert_eq!(prev.keys, "<S-Tab>");
        assert_eq!(prev.command_id, ids::JUMP_PREV);
    }

    #[test]
    fn test_keybindings_cancel() {
        let module = SnippetModule::new();
        let bindings = module.keybindings();
        let cancel = &bindings[2];
        assert_eq!(cancel.keys, "<Esc>");
        assert_eq!(cancel.command_id, ids::CANCEL);
    }

    // =========================================================================
    // CommandProvider
    // =========================================================================

    #[test]
    fn test_command_provider_returns_handlers() {
        let module = SnippetModule::new();
        let handlers = module.command_handlers();
        assert_eq!(handlers.len(), 6);
    }

    #[test]
    fn test_command_provider_matches_all_commands() {
        use std::path::PathBuf;
        let handle = SnippetRegistryHandle::new(SnippetRegistry::new());
        let fallback = reovim_kernel::api::v1::ModeId::new(ModuleId::new("test"), "insert");
        let module_handlers = SnippetModule::new().command_handlers();
        let all = command::all_commands(handle, fallback, PathBuf::from("/tmp"));
        assert_eq!(module_handlers.len(), all.len());
    }

    // =========================================================================
    // Dependencies
    // =========================================================================

    #[test]
    fn test_dependencies_default_empty() {
        let module = SnippetModule::new();
        assert!(module.dependencies().is_empty());
    }

    // =========================================================================
    // Init with real context
    // =========================================================================

    /// Register a mock parent mode and `SnippetParentMode` config
    /// (adapter initializes before snippet).
    fn register_mock_parent(services: &Arc<reovim_kernel::api::v1::ServiceRegistry>) {
        use reovim_kernel::api::v1::ModeId;
        let parent = ModeId::new(ModuleId::new("test"), "insert");
        let modes = services.get_or_create::<ModeInfoStore>();
        modes.add(ModeInfo {
            id: parent.clone(),
            display_name: "INSERT",
            cursor_style: CursorStyle::Bar,
            accepts_char_input: true,
            has_selection: false,
            inherits_from: None,
            is_entry: false,
        });
        services.register(Arc::new(SnippetParentMode::new(parent)));
    }

    #[test]
    fn test_init_registers_commands() {
        use {
            reovim_kernel::api::v1::{EventBus, KernelContext, ServiceRegistry},
            std::path::PathBuf,
        };

        let event_bus = Arc::new(EventBus::new());
        let services = Arc::new(ServiceRegistry::new());
        register_mock_parent(&services);

        let kernel = KernelContext::with_event_bus_services_and_options(
            event_bus,
            Arc::clone(&services),
            Arc::new(reovim_kernel::api::v1::OptionRegistry::new()),
        );
        let ctx = ModuleContext::new(
            kernel,
            services,
            PathBuf::from("/tmp/reovim-snippet-test/data"),
            PathBuf::from("/tmp/reovim-snippet-test/cache"),
        );

        let mut module = SnippetModule::new();
        let result = module.init(&ctx);
        assert_eq!(result, ProbeResult::Success);
        assert!(module.handle.is_some());

        // Verify CommandHandlerStore is populated
        let store = ctx.services.get::<CommandHandlerStore>();
        assert!(store.is_some());

        // Verify ResolverRegistry is populated
        let resolvers = ctx.services.get::<ResolverRegistry>();
        assert!(resolvers.is_some());

        // Verify ModeInfoStore is populated
        let modes = ctx.services.get::<ModeInfoStore>();
        assert!(modes.is_some());
    }

    #[test]
    fn test_init_then_exit() {
        use {
            reovim_kernel::api::v1::{EventBus, KernelContext, ServiceRegistry},
            std::path::PathBuf,
        };

        let event_bus = Arc::new(EventBus::new());
        let services = Arc::new(ServiceRegistry::new());
        register_mock_parent(&services);

        let kernel = KernelContext::with_event_bus_services_and_options(
            event_bus,
            Arc::clone(&services),
            Arc::new(reovim_kernel::api::v1::OptionRegistry::new()),
        );
        let ctx = ModuleContext::new(
            kernel,
            services,
            PathBuf::from("/tmp/reovim-snippet-test/data"),
            PathBuf::from("/tmp/reovim-snippet-test/cache"),
        );

        let mut module = SnippetModule::new();
        module.init(&ctx);
        assert!(module.handle.is_some());
        module.exit().unwrap();
        assert!(module.handle.is_none());
    }

    // =========================================================================
    // build_registry (#529)
    // =========================================================================

    mod build_registry_tests {
        use {
            super::*,
            std::{
                io::Write,
                sync::atomic::{AtomicU32, Ordering},
            },
        };

        static COUNTER: AtomicU32 = AtomicU32::new(0);

        struct TempDir(std::path::PathBuf);

        impl TempDir {
            fn path(&self) -> &std::path::Path {
                &self.0
            }
        }

        impl Drop for TempDir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        fn make_temp_dir() -> TempDir {
            let id = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir()
                .join(format!("reovim-build-registry-{}-{id}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }

        #[test]
        fn test_build_registry_empty_dir() {
            let dir = make_temp_dir();
            let registry = build_registry(dir.path());
            assert!(registry.find_by_prefix("rust", "fn").is_none());
        }

        #[test]
        fn test_build_registry_user_dir() {
            let dir = make_temp_dir();
            let user_dir = dir.path().join("user");
            std::fs::create_dir_all(&user_dir).unwrap();
            let mut f = std::fs::File::create(user_dir.join("rust.json")).unwrap();
            writeln!(f, r#"{{ "function": {{ "prefix": "fn", "body": "fn $1() {{}}" }} }}"#)
                .unwrap();

            let registry = build_registry(dir.path());
            assert!(registry.find_by_prefix("rust", "fn").is_some());
        }

        #[test]
        fn test_build_registry_legacy_flat_layout() {
            let dir = make_temp_dir();
            // Legacy: snippets/ directory (no user/ dir)
            let snippets_dir = dir.path().join("snippets");
            std::fs::create_dir_all(&snippets_dir).unwrap();
            let mut f = std::fs::File::create(snippets_dir.join("global.json")).unwrap();
            writeln!(f, r#"{{ "test": {{ "prefix": "tst", "body": "TEST" }} }}"#).unwrap();

            let registry = build_registry(dir.path());
            // Legacy snippets/ loaded as user source
            assert!(registry.find_by_prefix("global", "tst").is_some());
        }

        #[test]
        fn test_build_registry_user_takes_precedence_over_legacy() {
            let dir = make_temp_dir();
            // Both user/ and snippets/ exist — user/ wins
            let user_dir = dir.path().join("user");
            std::fs::create_dir_all(&user_dir).unwrap();
            let mut f = std::fs::File::create(user_dir.join("global.json")).unwrap();
            writeln!(f, r#"{{ "from_user": {{ "prefix": "tst", "body": "USER" }} }}"#).unwrap();

            let snippets_dir = dir.path().join("snippets");
            std::fs::create_dir_all(&snippets_dir).unwrap();
            let mut f2 = std::fs::File::create(snippets_dir.join("global.json")).unwrap();
            writeln!(f2, r#"{{ "from_legacy": {{ "prefix": "tst", "body": "LEGACY" }} }}"#)
                .unwrap();

            let registry = build_registry(dir.path());
            let def = registry.find_by_prefix("global", "tst").unwrap();
            assert_eq!(def.name, "from_user");
        }

        #[test]
        fn test_build_registry_builtin_is_lowest_priority() {
            let dir = make_temp_dir();
            // User snippet
            let user_dir = dir.path().join("user");
            std::fs::create_dir_all(&user_dir).unwrap();
            let mut f = std::fs::File::create(user_dir.join("global.json")).unwrap();
            writeln!(f, r#"{{ "user_fn": {{ "prefix": "fn", "body": "USER" }} }}"#).unwrap();

            // Built-in snippet with same prefix
            let builtin_dir = dir.path().join("built-in");
            std::fs::create_dir_all(&builtin_dir).unwrap();
            let mut f2 = std::fs::File::create(builtin_dir.join("global.json")).unwrap();
            writeln!(f2, r#"{{ "builtin_fn": {{ "prefix": "fn", "body": "BUILTIN" }} }}"#).unwrap();

            let registry = build_registry(dir.path());
            // User wins over built-in
            let def = registry.find_by_prefix("global", "fn").unwrap();
            assert_eq!(def.name, "user_fn");
        }

        #[test]
        fn test_build_registry_builtin_found_when_no_user() {
            let dir = make_temp_dir();
            let builtin_dir = dir.path().join("built-in");
            std::fs::create_dir_all(&builtin_dir).unwrap();
            let mut f = std::fs::File::create(builtin_dir.join("rust.json")).unwrap();
            writeln!(f, r#"{{ "builtin": {{ "prefix": "fn", "body": "BUILTIN" }} }}"#).unwrap();

            let registry = build_registry(dir.path());
            let def = registry.find_by_prefix("rust", "fn").unwrap();
            assert_eq!(def.name, "builtin");
        }

        #[test]
        fn test_build_registry_friendly_snippets() {
            let dir = make_temp_dir();
            let friendly_dir = dir.path().join("friendly-snippets");
            std::fs::create_dir_all(friendly_dir.join("snippets")).unwrap();

            // Create package.json manifest
            let mut pkg = std::fs::File::create(friendly_dir.join("package.json")).unwrap();
            writeln!(
                pkg,
                r#"{{ "contributes": {{ "snippets": [{{ "language": "rust", "path": "./snippets/rust.json" }}] }} }}"#
            )
            .unwrap();

            // Create snippet file
            let mut snip = std::fs::File::create(friendly_dir.join("snippets/rust.json")).unwrap();
            writeln!(snip, r#"{{ "friendly_fn": {{ "prefix": "ffn", "body": "FRIENDLY" }} }}"#)
                .unwrap();

            let registry = build_registry(dir.path());
            let def = registry.find_by_prefix("rust", "ffn").unwrap();
            assert_eq!(def.name, "friendly_fn");
        }

        #[test]
        fn test_build_registry_friendly_without_package_json_ignored() {
            let dir = make_temp_dir();
            // Create friendly-snippets dir WITHOUT package.json
            let friendly_dir = dir.path().join("friendly-snippets");
            std::fs::create_dir_all(&friendly_dir).unwrap();

            let registry = build_registry(dir.path());
            assert!(registry.find_by_prefix("rust", "ffn").is_none());
        }

        #[test]
        fn test_build_registry_user_dir_with_invalid_json() {
            let dir = make_temp_dir();
            let user_dir = dir.path().join("user");
            std::fs::create_dir_all(&user_dir).unwrap();
            // Write invalid JSON to trigger Err branch
            let mut f = std::fs::File::create(user_dir.join("rust.json")).unwrap();
            writeln!(f, "{{invalid json!!!").unwrap();

            // Should not panic — Err is silently ignored
            let registry = build_registry(dir.path());
            assert!(registry.find_by_prefix("rust", "fn").is_none());
        }

        #[test]
        fn test_build_registry_friendly_with_bad_package_json() {
            let dir = make_temp_dir();
            let friendly_dir = dir.path().join("friendly-snippets");
            std::fs::create_dir_all(&friendly_dir).unwrap();
            // Create invalid package.json — exists() is true but load() returns Err
            let mut pkg = std::fs::File::create(friendly_dir.join("package.json")).unwrap();
            writeln!(pkg, "not valid json").unwrap();

            let registry = build_registry(dir.path());
            // friendly-snippets NOT loaded (bad JSON), but user/builtin empty providers registered
            // Provider count should be 2 (empty user + empty builtin), not 3
            assert_eq!(registry.provider_count(), 2);
        }

        #[test]
        fn test_build_registry_builtin_with_invalid_json() {
            let dir = make_temp_dir();
            let builtin_dir = dir.path().join("built-in");
            std::fs::create_dir_all(&builtin_dir).unwrap();
            let mut f = std::fs::File::create(builtin_dir.join("rust.json")).unwrap();
            writeln!(f, "{{not json").unwrap();

            let registry = build_registry(dir.path());
            assert!(registry.find_by_prefix("rust", "fn").is_none());
        }

        #[test]
        fn test_build_registry_all_three_sources() {
            let dir = make_temp_dir();

            // User
            let user_dir = dir.path().join("user");
            std::fs::create_dir_all(&user_dir).unwrap();
            let mut f = std::fs::File::create(user_dir.join("global.json")).unwrap();
            writeln!(f, r#"{{ "user_s": {{ "prefix": "us", "body": "USER" }} }}"#).unwrap();

            // Friendly
            let friendly_dir = dir.path().join("friendly-snippets");
            std::fs::create_dir_all(friendly_dir.join("snippets")).unwrap();
            let mut pkg = std::fs::File::create(friendly_dir.join("package.json")).unwrap();
            writeln!(
                pkg,
                r#"{{ "contributes": {{ "snippets": [{{ "language": "global", "path": "./snippets/global.json" }}] }} }}"#
            )
            .unwrap();
            let mut snip =
                std::fs::File::create(friendly_dir.join("snippets/global.json")).unwrap();
            writeln!(snip, r#"{{ "friendly_s": {{ "prefix": "fs", "body": "FRIENDLY" }} }}"#)
                .unwrap();

            // Built-in
            let builtin_dir = dir.path().join("built-in");
            std::fs::create_dir_all(&builtin_dir).unwrap();
            let mut f2 = std::fs::File::create(builtin_dir.join("global.json")).unwrap();
            writeln!(f2, r#"{{ "builtin_s": {{ "prefix": "bs", "body": "BUILTIN" }} }}"#).unwrap();

            let registry = build_registry(dir.path());
            assert_eq!(registry.provider_count(), 3);
            assert!(registry.find_by_prefix("global", "us").is_some());
            assert!(registry.find_by_prefix("global", "fs").is_some());
            assert!(registry.find_by_prefix("global", "bs").is_some());
        }
    }
}
