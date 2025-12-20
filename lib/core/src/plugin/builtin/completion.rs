//! Code completion plugin

use std::any::TypeId;

use crate::{
    command::builtin::{
        CompletionConfirmCommand, CompletionDismissCommand, CompletionNextCommand,
        CompletionPrevCommand, CompletionTriggerCommand,
    },
    plugin::{Plugin, PluginContext, PluginId},
};

use super::CorePlugin;

/// Code completion plugin
///
/// Provides auto-completion:
/// - Trigger completion popup
/// - Navigate suggestions
/// - Confirm/dismiss
pub struct CompletionPlugin;

impl Plugin for CompletionPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:completion")
    }

    fn name(&self) -> &'static str {
        "Completion"
    }

    fn description(&self) -> &'static str {
        "Auto-completion popup"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![TypeId::of::<CorePlugin>()]
    }

    fn build(&self, ctx: &mut PluginContext) {
        let _ = ctx.register_command(CompletionTriggerCommand);
        let _ = ctx.register_command(CompletionNextCommand);
        let _ = ctx.register_command(CompletionPrevCommand);
        let _ = ctx.register_command(CompletionConfirmCommand);
        let _ = ctx.register_command(CompletionDismissCommand);
    }
}
