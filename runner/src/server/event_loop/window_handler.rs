//! Window action handlers for the event loop.
//!
//! Handles window management commands: split, close, focus, cycle, resize.

use reovim_kernel::api::v1::events::{WindowClosed, WindowCreated, WindowFocused};

use {super::EventLoop, crate::server::AppState};

impl<F: reovim_driver_input::InputFallbackHandler<AppState>> EventLoop<F> {
    /// Handle a window action from window commands.
    pub(super) fn handle_window_action(&mut self, action: reovim_driver_command::WindowAction) {
        use reovim_driver_command::WindowAction;

        tracing::debug!(?action, "Handling window action");

        match action {
            WindowAction::SplitHorizontal => self.handle_split(true),
            WindowAction::SplitVertical => self.handle_split(false),
            WindowAction::CloseWindow => {
                let Some(active) = self.app.windows.active_window() else {
                    self.set_error("No active window");
                    return;
                };
                let window_id = active.raw() as u64;
                if self.app.windows.close_window(active) {
                    self.app.kernel.event_bus.emit(WindowClosed { window_id });
                    self.last_error = None;
                } else {
                    self.set_error("Cannot close last window");
                }
            }
            WindowAction::CloseOthers => {
                let Some(active) = self.app.windows.active_window() else {
                    self.set_error("No active window");
                    return;
                };
                let others: Vec<_> = self
                    .app
                    .windows
                    .windows()
                    .filter(|&id| id != active)
                    .collect();
                for id in others {
                    let window_id = id.raw() as u64;
                    if self.app.windows.close_window(id) {
                        self.app.kernel.event_bus.emit(WindowClosed { window_id });
                    }
                }
                self.last_error = None;
            }
            WindowAction::FocusDirection(direction) => {
                if let Some(new_focus) = self.app.windows.focus_direction(direction) {
                    let old_focus = self.app.windows.active_window();
                    self.app.windows.set_active_window(new_focus);
                    self.app.kernel.event_bus.emit(WindowFocused {
                        from: old_focus.map(|w| w.raw() as u64),
                        to: new_focus.raw() as u64,
                    });
                    self.last_error = None;
                }
            }
            WindowAction::CycleForward => self.handle_cycle(true),
            WindowAction::CycleBackward => self.handle_cycle(false),
            WindowAction::ResizeHeightIncrease
            | WindowAction::ResizeHeightDecrease
            | WindowAction::ResizeWidthIncrease
            | WindowAction::ResizeWidthDecrease
            | WindowAction::ResizeEqual => {
                tracing::debug!(?action, "Window resize action (deferred)");
                self.last_error = None;
            }
        }
    }

    /// Helper for window split operations.
    pub(super) fn handle_split(&mut self, horizontal: bool) {
        let Some(active) = self.app.windows.active_window() else {
            self.set_error("No active window");
            return;
        };
        let buffer_id = self.app.windows.get(active).and_then(|w| w.buffer_id);
        let new_id = if horizontal {
            self.app.windows.split_horizontal(active)
        } else {
            self.app.windows.split_vertical(active)
        };
        let Some(new_id) = new_id else {
            self.set_error("Failed to split window");
            return;
        };
        // New window gets same buffer
        if let Some(buffer_id) = buffer_id
            && let Some(w) = self.app.windows.get_mut(new_id)
        {
            w.buffer_id = Some(buffer_id);
        }
        self.app.kernel.event_bus.emit(WindowCreated {
            window_id: new_id.raw() as u64,
        });
        self.last_error = None;
    }

    /// Helper for window cycle operations.
    pub(super) fn handle_cycle(&mut self, forward: bool) {
        let old_focus = self.app.windows.active_window();
        let changed = if forward {
            self.app.windows.cycle_forward()
        } else {
            self.app.windows.cycle_backward()
        };
        if changed {
            if let Some(new_focus) = self.app.windows.active_window() {
                self.app.kernel.event_bus.emit(WindowFocused {
                    from: old_focus.map(|w| w.raw() as u64),
                    to: new_focus.raw() as u64,
                });
            }
            self.last_error = None;
        }
    }
}
