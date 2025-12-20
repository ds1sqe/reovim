//! Leap motion plugin

use std::any::TypeId;

use crate::{
    command::builtin::{LeapBackwardCommand, LeapCancelCommand, LeapForwardCommand},
    plugin::{Plugin, PluginContext, PluginId},
};

use super::CorePlugin;

/// Leap motion plugin
///
/// Provides two-character jump navigation:
/// - s/S for forward/backward leap
/// - Character-based label selection
pub struct LeapPlugin;

impl Plugin for LeapPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:leap")
    }

    fn name(&self) -> &'static str {
        "Leap"
    }

    fn description(&self) -> &'static str {
        "Two-character jump navigation"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![TypeId::of::<CorePlugin>()]
    }

    fn build(&self, ctx: &mut PluginContext) {
        let _ = ctx.register_command(LeapForwardCommand);
        let _ = ctx.register_command(LeapBackwardCommand);
        let _ = ctx.register_command(LeapCancelCommand);
    }
}
