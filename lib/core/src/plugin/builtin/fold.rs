//! Code folding plugin

use std::any::TypeId;

use crate::{
    command::builtin::{
        FoldCloseAllCommand, FoldCloseCommand, FoldOpenAllCommand, FoldOpenCommand,
        FoldToggleCommand,
    },
    plugin::{Plugin, PluginContext, PluginId},
};

use super::CorePlugin;

/// Code folding plugin
///
/// Provides code folding:
/// - Toggle, open, close folds
/// - Open/close all folds
pub struct FoldPlugin;

impl Plugin for FoldPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:fold")
    }

    fn name(&self) -> &'static str {
        "Fold"
    }

    fn description(&self) -> &'static str {
        "Code folding"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![TypeId::of::<CorePlugin>()]
    }

    fn build(&self, ctx: &mut PluginContext) {
        let _ = ctx.register_command(FoldToggleCommand);
        let _ = ctx.register_command(FoldOpenCommand);
        let _ = ctx.register_command(FoldCloseCommand);
        let _ = ctx.register_command(FoldOpenAllCommand);
        let _ = ctx.register_command(FoldCloseAllCommand);
    }
}
