#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Microscope fuzzy finder module - POLICY layer.
//!
//! Orchestrates the fuzzy finder UI: provides session state management,
//! command handlers, and bridges state to clients.
//!
//! # Architecture (#522)
//!
//! This module reads from driver-picker's `PickerRegistry` and owns the
//! per-client `MicroscopeState` stored in `ExtensionMap`. The bridge
//! serializes state to JSON consumed by both TUI and Web extensions.
//!
//! Picker data sources (files, buffers, grep, commands) live in separate
//! `reovim-picker-*` crates. Each picker implements `Picker::execute()`
//! for its own action dispatch.

pub mod bridge;
pub mod commands;
pub mod ids;
pub mod modes;
pub mod resolver;
pub mod state;

pub use {bridge::MicroscopeBridge, state::MicroscopeState};

const KIND: &str = "microscope";

use {
    reovim_driver_command::CommandHandlerStore,
    reovim_driver_input::{KeybindingStore, ModeInfo, ModeInfoStore, ResolverRegistry},
    reovim_driver_session::bridges::BridgeProvider,
    reovim_kernel::api::v1::{
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, OptionConstraint,
        OptionSpec, OptionValue, ProbeResult, Version,
    },
};

/// Microscope fuzzy finder module.
///
/// Registers [`MicroscopeBridge`] and commands, modes, and keybindings
/// for the fuzzy finder. Picker data sources are registered by separate
/// `reovim-picker-*` modules.
pub struct MicroscopeModule;

impl MicroscopeModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for MicroscopeModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for MicroscopeModule {
    fn id(&self) -> ModuleId {
        ids::MODULE
    }

    fn name(&self) -> &'static str {
        "Microscope"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register MicroscopeBridge via BridgeProvider.
        let provider = ctx.services.get_or_create::<BridgeProvider>();
        provider.register(MicroscopeBridge);

        // Register modes.
        let mode_store = ctx.services.get_or_create::<ModeInfoStore>();
        for mode in modes::MicroscopeMode::ALL {
            mode_store.add(ModeInfo::from_mode(*mode));
        }

        // Register resolver for microscope picker mode.
        let resolver_registry = ctx.services.get_or_create::<ResolverRegistry>();
        resolver_registry.register(resolver::MicroscopeResolver::new());

        // Register command handlers.
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in commands::command_handlers() {
            command_store.add(handler);
        }

        // Register keybindings.
        let keybinding_store = ctx.services.get_or_create::<KeybindingStore>();
        keybinding_store.add_all(self.keybindings());

        // Epic #570: Register microscope options (#574)
        for spec in microscope_option_specs() {
            if let Err(e) = ctx.kernel.options.register(spec) {
                return ProbeResult::Failed(ModuleError::InitFailed(format!(
                    "Failed to register microscope option: {e}"
                )));
            }
        }

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn extension_kinds(&self) -> &[&'static str] {
        &[KIND]
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        vec![
            // Microscope mode: navigation
            KeybindingRegistration::new("<C-n>", ids::NEXT_ITEM)
                .with_modes(&["microscope:MICROSCOPE"])
                .with_description("Next item"),
            KeybindingRegistration::new("<Down>", ids::NEXT_ITEM)
                .with_modes(&["microscope:MICROSCOPE"])
                .with_description("Next item"),
            KeybindingRegistration::new("<C-p>", ids::PREV_ITEM)
                .with_modes(&["microscope:MICROSCOPE"])
                .with_description("Previous item"),
            KeybindingRegistration::new("<Up>", ids::PREV_ITEM)
                .with_modes(&["microscope:MICROSCOPE"])
                .with_description("Previous item"),
            // Microscope mode: actions
            KeybindingRegistration::new("<CR>", ids::SELECT_ITEM)
                .with_modes(&["microscope:MICROSCOPE"])
                .with_description("Select item"),
            KeybindingRegistration::new("<Esc>", ids::CLOSE)
                .with_modes(&["microscope:MICROSCOPE"])
                .with_description("Close picker"),
            KeybindingRegistration::new("<BS>", ids::BACKSPACE)
                .with_modes(&["microscope:MICROSCOPE"])
                .with_description("Delete character"),
        ]
    }
}

/// Microscope option specifications.
///
/// Picker layout, preview, search, and prompt options.
/// Registered during `MicroscopeModule::init()`.
#[cfg_attr(coverage_nightly, coverage(off))]
fn microscope_option_specs() -> Vec<OptionSpec> {
    vec![
        OptionSpec::new(
            "picker_height",
            "Maximum height of the picker window",
            OptionValue::int(15),
        )
        .with_constraint(OptionConstraint::range(3, 50))
        .with_owner(ids::MODULE),
        OptionSpec::new("picker_preview", "Show preview pane in picker", OptionValue::bool(true))
            .with_owner(ids::MODULE),
        OptionSpec::new(
            "picker_ignorecase",
            "Ignore case in picker search",
            OptionValue::bool(true),
        )
        .with_owner(ids::MODULE),
        OptionSpec::new(
            "picker_border",
            "Border style for picker window",
            OptionValue::choice(
                "rounded",
                vec![
                    "none".to_string(),
                    "single".to_string(),
                    "double".to_string(),
                    "rounded".to_string(),
                ],
            ),
        )
        .with_owner(ids::MODULE),
        OptionSpec::new(
            "picker_prompt",
            "Prompt string shown in picker input",
            OptionValue::string("> "),
        )
        .with_constraint(OptionConstraint::string_length(0, 10))
        .with_owner(ids::MODULE),
    ]
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(MicroscopeModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
