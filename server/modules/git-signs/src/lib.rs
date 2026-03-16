#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Git signs module for reovim.
//!
//! Provides gutter annotations for git diff hunks: additions, changes,
//! and deletions. Also provides hunk navigation, stage, reset, preview,
//! and diff commands (#668).

use std::sync::Arc;

use {
    reovim_driver_annotation::{AnnotationSourceKey, AnnotationSourceRegistry},
    reovim_driver_command::CommandHandlerStore,
    reovim_driver_input::KeybindingStore,
    reovim_kernel::api::v1::{
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
};

pub mod commands;
mod hunk;
pub mod ids;
pub mod navigation;
mod source;

pub use {hunk::SignKind, source::GitSignsSource};

/// Git signs module.
///
/// Registers a `GitSignsSource` into the `AnnotationSourceRegistry`
/// so git diff hunks appear as gutter signs. Also registers hunk
/// navigation and operation commands (#668).
pub struct GitSignsModule;

impl GitSignsModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for GitSignsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for GitSignsModule {
    fn id(&self) -> ModuleId {
        ids::MODULE
    }

    fn name(&self) -> &'static str {
        "Git Signs"
    }

    fn version(&self) -> Version {
        Version::new(0, 2, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register annotation source for gutter signs
        let source_registry = ctx.services.get_or_create::<AnnotationSourceRegistry>();
        source_registry.register(
            AnnotationSourceKey::new("git-signs"),
            Arc::new(GitSignsSource::new(ctx.services.clone())),
        );

        // Register command handlers (#668)
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in commands::command_handlers() {
            command_store.add(handler);
        }

        // Register keybindings (#668)
        let keybinding_store = ctx.services.get_or_create::<KeybindingStore>();
        keybinding_store.add_all(self.keybindings());

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        vec![
            // Hunk navigation
            KeybindingRegistration::new("]h", ids::NEXT_HUNK)
                .with_modes(&["normal"])
                .with_description("Next git hunk"),
            KeybindingRegistration::new("[h", ids::PREV_HUNK)
                .with_modes(&["normal"])
                .with_description("Previous git hunk"),
            // Hunk operations
            KeybindingRegistration::new("<leader>ghs", ids::STAGE_HUNK)
                .with_modes(&["normal"])
                .with_description("Stage hunk"),
            KeybindingRegistration::new("<leader>ghr", ids::RESET_HUNK)
                .with_modes(&["normal"])
                .with_description("Reset hunk"),
            KeybindingRegistration::new("<leader>ghS", ids::STAGE_BUFFER)
                .with_modes(&["normal"])
                .with_description("Stage buffer"),
            KeybindingRegistration::new("<leader>ghR", ids::RESET_BUFFER)
                .with_modes(&["normal"])
                .with_description("Reset buffer"),
            KeybindingRegistration::new("<leader>ghu", ids::UNSTAGE_FILE)
                .with_modes(&["normal"])
                .with_description("Unstage file"),
            // Preview
            KeybindingRegistration::new("<leader>ghp", ids::PREVIEW_HUNK)
                .with_modes(&["normal"])
                .with_description("Preview hunk"),
            KeybindingRegistration::new("<leader>ghd", ids::DIFF_THIS)
                .with_modes(&["normal"])
                .with_description("Diff this file"),
        ]
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(GitSignsModule);

#[cfg(test)]
mod tests;
