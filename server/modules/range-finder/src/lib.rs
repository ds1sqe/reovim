#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Range-finder module - POLICY layer.
//!
//! This module provides jump navigation and code folding:
//! - **Jump navigation**: `s{char}{char}` two-char search with label overlay
//! - **Code folding**: `za`/`zo`/`zc`/`zR`/`zM` fold commands
//!
//! # Architecture (#524)
//!
//! Jump state (`JumpSessionState`) and fold state (`FoldSessionState`) are
//! independent `SessionExtension` types. Jump labels are rendered by client
//! extensions via `ExtensionStateBridge`.
//!
//! The module registers its own mode (`range-finder:jump-input`) for label
//! selection. The parent mode for keybinding inheritance is injected by
//! the personality manifest (e.g., `vim.toml`) via [`ModeBridgeStore`].

pub mod find_char;
pub mod fold;
pub mod jump;

use {
    reovim_driver_input::{ModeInfo, ModeInfoStore, ResolverRegistry},
    reovim_driver_manifest::ModeBridgeStore,
    reovim_kernel::api::v1::{
        CursorStyle, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
};

pub(crate) const KIND_JUMP: &str = "range-finder-jump";
pub(crate) const KIND_FOLD: &str = "range-finder-fold";
const MODULE_ID: ModuleId = ModuleId::new("range-finder");

/// Range-finder module providing jump navigation and code folding.
pub struct RangeFinderModule;

impl RangeFinderModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for RangeFinderModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for RangeFinderModule {
    fn id(&self) -> ModuleId {
        MODULE_ID
    }

    fn name(&self) -> &'static str {
        "range-finder"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn dependencies(&self) -> Vec<ModuleId> {
        vec![]
    }

    fn optional_dependencies(&self) -> Vec<ModuleId> {
        // Personality modules (e.g., vim) populate ModeBridgeStore before us.
        // Optional: range-finder still works without a personality, just no parent mode bridging.
        vec![ModuleId::new("vim")]
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register bridges (#524)
        let provider = ctx
            .services
            .get_or_create::<reovim_driver_session::bridges::BridgeProvider>();
        provider.register(jump::bridge::JumpBridge);
        provider.register(fold::bridge::FoldBridge);

        // Register jump commands (#524)
        let command_store = ctx
            .services
            .get_or_create::<reovim_driver_command::CommandHandlerStore>();
        for handler in jump::command::all_commands() {
            command_store.add(handler);
        }

        // Register fold commands (#524)
        for handler in fold::command::all_commands() {
            command_store.add(handler);
        }

        // #585: Read parent mode from ModeBridgeStore (populated by personality module)
        // instead of requiring adapter pre-registration.
        let modes = ctx.services.get_or_create::<ModeInfoStore>();
        let parent_mode = resolve_parent_mode(ctx, &modes);

        if parent_mode.is_some() {
            // Register enhanced find-char command (#535) — overrides vim's basic handler
            command_store.add(Box::new(find_char::EnhancedFindCharCommand));
        }

        // Register jump resolver for jump-input mode (#524)
        let resolvers = ctx.services.get_or_create::<ResolverRegistry>();
        resolvers.register(jump::resolver::JumpResolver::with_parent(
            parent_mode.clone().unwrap_or(jump::ids::JUMP_INPUT_MODE),
        ));

        // Register mode info for jump-input mode (#524)
        modes.add(ModeInfo {
            id: jump::ids::JUMP_INPUT_MODE,
            display_name: "JUMP",
            cursor_style: CursorStyle::Block,
            accepts_char_input: true,
            has_selection: false,
            inherits_from: parent_mode,
            is_entry: false,
        });

        ProbeResult::Success
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn extension_kinds(&self) -> &[&'static str] {
        &[KIND_JUMP, KIND_FOLD]
    }
}

/// Resolve the parent mode for jump-input from `ModeBridgeStore`.
///
/// Returns `None` if no personality module is loaded (reduced functionality).
fn resolve_parent_mode(
    ctx: &ModuleContext,
    modes: &ModeInfoStore,
) -> Option<reovim_kernel::api::v1::ModeId> {
    let bridge_store = ctx.services.get::<ModeBridgeStore>()?;
    let parent_str = bridge_store.find_parent("range-finder:jump-input")?;
    let (module, name) = parent_str.split_once(':')?;

    if let Some(mode_id) = modes.find_by_name(module, name) {
        return Some(mode_id);
    }
    tracing::warn!("Mode bridge parent '{parent_str}' not found in ModeInfoStore");
    None
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(RangeFinderModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
