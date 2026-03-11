//! Vim keybinding adapter for snippet commands.
//!
//! This adapter module bridges `vim` (editor personality) and
//! `snippet` (snippet expansion engine). It maps vim mode keys
//! to snippet commands:
//! - `<C-s>` → Expand snippet at cursor (insert mode)
//! - `<Space>sc` → List available snippets (normal mode)
//! - `<Space>sr` → Reload snippet files (normal mode)
//!
//! # Architecture
//!
//! This is an adapter (integration) module. It exists because
//! `snippet` should not know about vim's mode names:
//! - `snippet` provides commands and snippet engine (no vim knowledge)
//! - `vim` provides modes and core keybindings (no snippet knowledge)
//! - This module bridges them with explicit, visible coupling
//!
//! Note: snippet's own-mode keybindings (`snippet:navigating`) remain
//! in the snippet module itself, as they target snippet's own mode.

use {
    reovim_driver_input::{KeybindingStore, ModeInfoStore},
    reovim_kernel::api::v1::{
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
    reovim_module_snippet::{SnippetParentMode, ids as snippet},
};

const MODULE: ModuleId = ModuleId::new("vim-snippet");

/// Vim-snippet keybinding adapter module.
///
/// Registers snippet expansion and management keybindings in
/// `vim:insert` and `vim:normal` modes, targeting commands
/// defined in `snippet`.
pub struct VimSnippetModule;

impl VimSnippetModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for VimSnippetModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for VimSnippetModule {
    fn id(&self) -> ModuleId {
        MODULE
    }

    fn name(&self) -> &'static str {
        "Vim Snippet Keybindings"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Resolve vim:insert and register as SnippetParentMode for snippet
        let modes = ctx.services.get_or_create::<ModeInfoStore>();
        let vim_insert = modes
            .find_by_name("vim", "insert")
            .expect("vim:insert must be registered before vim-snippet");
        ctx.services
            .register(std::sync::Arc::new(SnippetParentMode::new(vim_insert)));

        let store = ctx.services.get_or_create::<KeybindingStore>();
        store.add_all(self.keybindings());
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        vec![
            KeybindingRegistration::new("<C-s>", snippet::EXPAND)
                .with_modes(&["vim:insert"])
                .with_category("snippet")
                .with_description("Expand snippet at cursor"),
            KeybindingRegistration::new("<Space>sc", snippet::CATALOG)
                .with_modes(&["vim:normal"])
                .with_category("snippet")
                .with_description("List available snippets"),
            KeybindingRegistration::new("<Space>sr", snippet::RELOAD)
                .with_modes(&["vim:normal"])
                .with_category("snippet")
                .with_description("Reload snippet files"),
        ]
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(VimSnippetModule);

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use super::*;

    #[test]
    fn module_id() {
        let module = VimSnippetModule::new();
        assert_eq!(module.id().as_str(), "vim-snippet");
    }

    #[test]
    fn module_name() {
        let module = VimSnippetModule::new();
        assert_eq!(module.name(), "Vim Snippet Keybindings");
    }

    #[test]
    fn module_version() {
        let module = VimSnippetModule::new();
        let v = module.version();
        assert_eq!((v.major, v.minor, v.patch), (0, 1, 0));
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn module_default() {
        let module = VimSnippetModule::default();
        assert_eq!(module.id().as_str(), "vim-snippet");
    }

    #[test]
    fn module_exit() {
        let mut module = VimSnippetModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn init_registers_keybindings_and_parent_mode() {
        use reovim_kernel::api::v1::{CursorStyle, ModeId, ServiceRegistry};

        let services = Arc::new(ServiceRegistry::new());

        // vim:insert must exist (vim initializes before vim-snippet)
        let modes = services.get_or_create::<ModeInfoStore>();
        modes.add(reovim_driver_input::ModeInfo {
            id: ModeId::new(ModuleId::new("vim"), "insert"),
            display_name: "INSERT",
            cursor_style: CursorStyle::Bar,
            accepts_char_input: true,
            has_selection: false,
            inherits_from: None,
            is_entry: false,
        });

        let ctx = test_module_context(services.clone());

        let mut module = VimSnippetModule::new();
        let result = module.init(&ctx);
        assert!(matches!(result, ProbeResult::Success));

        let store = services.get::<KeybindingStore>();
        assert!(store.is_some());

        // Verify SnippetParentMode was registered
        let parent = services.get::<SnippetParentMode>();
        assert!(parent.is_some());
    }

    #[test]
    fn keybindings_count() {
        let module = VimSnippetModule::new();
        assert_eq!(module.keybindings().len(), 3);
    }

    #[test]
    fn keybindings_keys() {
        let module = VimSnippetModule::new();
        let keys: Vec<_> = module.keybindings().iter().map(|kb| kb.keys).collect();
        assert!(keys.contains(&"<C-s>"));
        assert!(keys.contains(&"<Space>sc"));
        assert!(keys.contains(&"<Space>sr"));
    }

    #[test]
    fn keybindings_target_correct_modes() {
        let module = VimSnippetModule::new();
        let bindings = module.keybindings();
        // <C-s> targets vim:insert
        assert!(bindings[0].modes.contains(&"vim:insert"));
        // <Space>sc and <Space>sr target vim:normal
        assert!(bindings[1].modes.contains(&"vim:normal"));
        assert!(bindings[2].modes.contains(&"vim:normal"));
    }

    #[test]
    fn keybindings_have_snippet_category() {
        let module = VimSnippetModule::new();
        for kb in module.keybindings() {
            assert_eq!(kb.category, Some("snippet"));
        }
    }

    #[test]
    fn keybindings_have_descriptions() {
        let module = VimSnippetModule::new();
        for kb in module.keybindings() {
            assert!(!kb.description.is_empty());
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_module_context(
        services: Arc<reovim_kernel::api::v1::ServiceRegistry>,
    ) -> ModuleContext {
        ModuleContext::new(
            reovim_kernel::api::v1::KernelContext::default(),
            services,
            PathBuf::from("/tmp"),
            PathBuf::from("/tmp"),
        )
    }
}
