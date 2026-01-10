use std::sync::Arc;

use crate::{
    bind::CommandRef,
    command::{CommandContext, id::builtin, traits::OperatorMotionAction},
    event::{CommandEvent, RuntimeEvent, TextInputEvent},
    event_bus::{
        EventResult,
        core_events::{
            ProfileLoadEvent, ProfileSaveEvent, RegisterConfigurable, RegisterExCommand,
            RequestApplyCmdlineCompletion, RequestCursorMove, RequestFocusChange,
            RequestInsertText, RequestModeChange, RequestOpenFile, RequestOpenFileAtPosition,
            RequestSetIndentGuide, RequestSetLineNumbers, RequestSetRegister,
            RequestSetRelativeLineNumbers, RequestSetScrollbar, RequestSetSignColumn,
            RequestSetTheme,
        },
    },
    modd::{OperatorType, SubMode},
    motion::Motion,
};

use super::Runtime;

impl Runtime {
    /// Register all event subscriptions from the event bus.
    /// This method is called during Runtime initialization to wire up
    /// all the event handlers for plugin communication.
    pub(super) fn register_event_subscribers(&self) {
        self.register_focus_change_handler();
        self.register_mode_change_handler();
        self.register_file_operations_handlers();
        self.register_register_handler();
        self.register_ex_command_handler();
        self.register_profile_handlers();
        self.register_text_operations_handlers();
        self.register_cursor_move_handler();
        self.register_settings_handlers();
        self.register_cmdline_completion_handler();
    }

    /// Subscribe to focus change requests from plugins.
    /// HIGH PRIORITY: Focus changes are user-visible and should be processed immediately.
    fn register_focus_change_handler(&self) {
        let mode_tx = self.mode_tx.clone();
        let hi_tx = self.hi_tx.clone();
        self.event_bus
            .subscribe::<RequestFocusChange, _>(100, move |event, _ctx| {
                let current_mode = mode_tx.borrow().clone();
                let new_mode = current_mode.set_interactor_id(event.target);
                // Immediately update mode_tx so CommandHandler sees the new mode
                // before processing subsequent keys (fixes race condition)
                let _ = mode_tx.send(new_mode.clone());
                // Also send through event loop to update runtime.mode_state
                let _ = hi_tx.try_send(RuntimeEvent::mode_change(new_mode));
                tracing::info!(
                    "Runtime: Requesting focus change to component '{}'",
                    event.target.0
                );
                EventResult::Handled
            });
    }

    /// Subscribe to mode change requests from plugins.
    /// HIGH PRIORITY: Mode changes are user-visible and should be processed immediately.
    fn register_mode_change_handler(&self) {
        let mode_tx = self.mode_tx.clone();
        let hi_tx = self.hi_tx.clone();
        self.event_bus
            .subscribe::<RequestModeChange, _>(100, move |event, _ctx| {
                // Immediately update mode_tx so CommandHandler sees the new mode
                // before processing subsequent keys (fixes race condition)
                let _ = mode_tx.send(event.mode.clone());
                // Also send through event loop to update runtime.mode_state
                let _ = hi_tx.try_send(RuntimeEvent::mode_change(event.mode.clone()));
                tracing::info!(
                    "Runtime: Requesting mode change to interactor='{}', edit_mode={:?}, sub_mode={:?}",
                    event.mode.interactor_id.0,
                    event.mode.edit_mode,
                    event.mode.sub_mode
                );
                EventResult::Handled
            });
    }

    /// Subscribe to file operation requests from plugins.
    /// LOW PRIORITY: File loading is a background operation.
    fn register_file_operations_handlers(&self) {
        // RequestOpenFile handler
        {
            let lo_tx = self.lo_tx.clone();
            self.event_bus
                .subscribe::<RequestOpenFile, _>(100, move |event, _ctx| {
                    tracing::info!("Runtime: Requesting to open file: {:?}", event.path);
                    // Send OpenFileRequest to the runtime event loop
                    let _ = lo_tx.try_send(RuntimeEvent::open_file(event.path.clone()));
                    EventResult::Handled
                });
        }

        // RequestOpenFileAtPosition handler (LSP navigation)
        {
            let lo_tx = self.lo_tx.clone();
            self.event_bus
                .subscribe::<RequestOpenFileAtPosition, _>(100, move |event, _ctx| {
                    tracing::info!(
                        "Runtime: Requesting to open file at position: {:?}:{}:{}",
                        event.path,
                        event.line,
                        event.column
                    );
                    let _ = lo_tx.try_send(RuntimeEvent::open_file_at(
                        event.path.clone(),
                        event.line,
                        event.column,
                    ));
                    EventResult::Handled
                });
        }
    }

    /// Subscribe to register set requests from plugins.
    /// LOW PRIORITY: Register operations are background tasks.
    fn register_register_handler(&self) {
        let lo_tx = self.lo_tx.clone();
        self.event_bus
            .subscribe::<RequestSetRegister, _>(100, move |event, _ctx| {
                tracing::debug!("Runtime: Requesting to set register {:?}", event.register);
                let _ =
                    lo_tx.try_send(RuntimeEvent::set_register(event.register, event.text.clone()));
                EventResult::Handled
            });
    }

    /// Subscribe to ex-command registration from plugins.
    fn register_ex_command_handler(&self) {
        let registry = Arc::clone(&self.ex_command_registry);
        self.event_bus
            .subscribe::<RegisterExCommand, _>(50, move |event, _ctx| {
                registry.register(event.name.to_string(), event.handler.clone());
                tracing::debug!("Runtime: Registered ex-command '{}'", event.name);
                EventResult::Handled
            });
    }

    /// Subscribe to profile-related events.
    fn register_profile_handlers(&self) {
        // RegisterConfigurable handler
        {
            let registry = Arc::clone(&self.profile_registry);
            self.event_bus
                .subscribe::<RegisterConfigurable, _>(40, move |event, _ctx| {
                    registry.register(event.component.clone());
                    EventResult::Handled
                });
        }

        // ProfileLoadEvent handler
        {
            self.event_bus
                .subscribe::<ProfileLoadEvent, _>(100, move |event, ctx| {
                    // Request render after profile load
                    ctx.request_render();
                    tracing::info!(profile = %event.name, "Profile load requested (full implementation pending)");
                    // TODO: Actually load the profile using profile_registry and profile_manager
                    // This requires Runtime to be mutable, which needs architectural changes
                    EventResult::Handled
                });
        }

        // ProfileSaveEvent handler
        {
            self.event_bus
                .subscribe::<ProfileSaveEvent, _>(100, move |event, _ctx| {
                    tracing::info!(profile = %event.name, "Profile save requested (full implementation pending)");
                    // TODO: Actually save the profile using profile_registry and profile_manager
                    // This requires Runtime to be mutable, which needs architectural changes
                    EventResult::Handled
                });
        }
    }

    /// Subscribe to text insert requests from plugins (for auto-pair insertion).
    /// HIGH PRIORITY: Auto-pair and completion insertions must be processed immediately
    /// to maintain <16ms latency target for user-initiated text input.
    fn register_text_operations_handlers(&self) {
        let hi_tx = self.hi_tx.clone();
        self.event_bus
            .subscribe::<RequestInsertText, _>(100, move |event, _ctx| {
                tracing::trace!("Runtime: Inserting text via plugin request: {:?}", event.text);

                // Delete prefix first (for completion: replace typed prefix with full word)
                for _ in 0..event.delete_prefix_len {
                    let _ = hi_tx
                        .try_send(RuntimeEvent::text_input(TextInputEvent::DeleteCharBackward));
                }

                // Insert each character
                for c in event.text.chars() {
                    let _ = hi_tx.try_send(RuntimeEvent::text_input(TextInputEvent::InsertChar(c)));
                }

                // If requested, move cursor left after insertion
                if event.move_cursor_left {
                    let _ = hi_tx.try_send(RuntimeEvent::command(CommandEvent {
                        command: CommandRef::Registered(builtin::CURSOR_LEFT),
                        context: CommandContext::default(),
                    }));
                }

                EventResult::Handled
            });
    }

    /// Subscribe to `RequestCursorMove` (for plugin-driven jumps).
    /// HIGH PRIORITY: Cursor movements are user-visible and should be processed immediately.
    fn register_cursor_move_handler(&self) {
        let hi_tx = self.hi_tx.clone();
        let mode_tx = self.mode_tx.clone();
        self.event_bus
            .subscribe::<RequestCursorMove, _>(100, move |event, _ctx| {
                tracing::trace!(
                    "Runtime: RequestCursorMove to line={}, col={} in buffer={}",
                    event.line,
                    event.column,
                    event.buffer_id
                );

                // Check for operator from motion_context first, then fallback to mode
                let operator_info = event.motion_context.as_ref().map_or_else(
                    || {
                        // Fallback: check if we're in operator-pending mode
                        let current_mode = mode_tx.borrow().clone();
                        match current_mode.sub_mode {
                            SubMode::OperatorPending { operator, count } => Some((operator, count)),
                            _ => None,
                        }
                    },
                    |motion_ctx| motion_ctx.operator.map(|op| (op, motion_ctx.count)),
                );

                match operator_info {
                    Some((operator, op_count)) => {
                        // Create operator motion action for jump
                        tracing::debug!(
                            "Runtime: Jump with operator {:?}, op_count={:?}",
                            operator,
                            op_count
                        );

                        let motion = Motion::JumpTo {
                            line: event.line,
                            column: event.column,
                        };

                        let action = match operator {
                            OperatorType::Delete => OperatorMotionAction::Delete {
                                motion,
                                count: op_count.unwrap_or(1),
                            },
                            OperatorType::Yank => OperatorMotionAction::Yank {
                                motion,
                                count: op_count.unwrap_or(1),
                            },
                            OperatorType::Change => OperatorMotionAction::Change {
                                motion,
                                count: op_count.unwrap_or(1),
                            },
                        };

                        // Send operator motion event (runtime will handle mode change)
                        let _ = hi_tx.try_send(RuntimeEvent::operator_motion(action));
                    }
                    None => {
                        // Normal cursor move (no operator)
                        let _ = hi_tx.try_send(RuntimeEvent::move_cursor(
                            event.buffer_id,
                            event.line,
                            event.column,
                        ));
                    }
                }

                EventResult::Handled
            });
    }

    /// Subscribe to settings/option capability requests.
    /// LOW PRIORITY: Settings changes are background operations.
    fn register_settings_handlers(&self) {
        // RequestSetLineNumbers handler
        {
            let lo_tx = self.lo_tx.clone();
            self.event_bus
                .subscribe::<RequestSetLineNumbers, _>(100, move |event, _ctx| {
                    let _ = lo_tx.try_send(RuntimeEvent::set_line_numbers(event.enabled));
                    EventResult::Handled
                });
        }

        // RequestSetRelativeLineNumbers handler
        {
            let lo_tx = self.lo_tx.clone();
            self.event_bus
                .subscribe::<RequestSetRelativeLineNumbers, _>(100, move |event, _ctx| {
                    let _ = lo_tx.try_send(RuntimeEvent::set_relative_line_numbers(event.enabled));
                    EventResult::Handled
                });
        }

        // RequestSetTheme handler
        {
            let lo_tx = self.lo_tx.clone();
            self.event_bus
                .subscribe::<RequestSetTheme, _>(100, move |event, _ctx| {
                    let _ = lo_tx.try_send(RuntimeEvent::set_theme(event.name.clone()));
                    EventResult::Handled
                });
        }

        // RequestSetScrollbar handler
        {
            let lo_tx = self.lo_tx.clone();
            self.event_bus
                .subscribe::<RequestSetScrollbar, _>(100, move |event, _ctx| {
                    let _ = lo_tx.try_send(RuntimeEvent::set_scrollbar(event.enabled));
                    EventResult::Handled
                });
        }

        // RequestSetIndentGuide handler
        {
            let lo_tx = self.lo_tx.clone();
            self.event_bus
                .subscribe::<RequestSetIndentGuide, _>(100, move |event, _ctx| {
                    let _ = lo_tx.try_send(RuntimeEvent::set_indent_guide(event.enabled));
                    EventResult::Handled
                });
        }

        // RequestSetSignColumn handler
        {
            let lo_tx = self.lo_tx.clone();
            self.event_bus
                .subscribe::<RequestSetSignColumn, _>(100, move |event, _ctx| {
                    let _ = lo_tx.try_send(RuntimeEvent::set_sign_column(event.mode));
                    EventResult::Handled
                });
        }
    }

    /// Subscribe to command line completion apply requests.
    /// HIGH PRIORITY: Command line completion is user-initiated and should be processed immediately.
    fn register_cmdline_completion_handler(&self) {
        let hi_tx = self.hi_tx.clone();
        self.event_bus
            .subscribe::<RequestApplyCmdlineCompletion, _>(100, move |event, _ctx| {
                let _ = hi_tx.try_send(RuntimeEvent::apply_cmdline_completion(
                    event.text.clone(),
                    event.replace_start,
                ));
                EventResult::Handled
            });
    }
}
