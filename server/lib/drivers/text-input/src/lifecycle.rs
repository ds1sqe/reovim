//! Mode lifecycle callbacks.

use reovim_kernel::api::v1::ModeId;

/// Mode lifecycle handler trait.
pub trait ModeLifecycleHandler: Send + Sync {
    /// Called when entering an input-accepting mode.
    fn on_input_mode_enter(&mut self, mode: &ModeId);

    /// Called when exiting an input-accepting mode.
    fn on_input_mode_exit(&mut self, mode: &ModeId);

    /// Called on any mode transition.
    fn on_mode_change(&mut self, from: &ModeId, to: &ModeId) {
        let _ = (from, to);
    }

    /// Called when exiting a mode (before pop/set).
    fn on_mode_exit(&mut self, mode: &ModeId) {
        let _ = mode;
    }
}

/// No-op lifecycle handler for testing.
#[derive(Debug, Default, Clone, Copy)]
pub struct NopLifecycleHandler;

impl ModeLifecycleHandler for NopLifecycleHandler {
    fn on_input_mode_enter(&mut self, _mode: &ModeId) {}
    fn on_input_mode_exit(&mut self, _mode: &ModeId) {}
}
