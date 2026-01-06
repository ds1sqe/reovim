//! Range-Finder Plugin for reovim
//!
//! Unified jump navigation and code folding system:
//! - Jump navigation: Multi-char search (`s`), enhanced f/t motions
//! - Code folding: Toggle/open/close folds (`za`, `zo`, `zc`, `zR`, `zM`)

mod fold;
mod jump;

use std::sync::Arc;

use {
    jump::input::{InputResult, handle_char_input},
    reovim_core::{
        bind::{CommandRef, KeymapScope, SubModeKind},
        command::id::CommandId,
        event_bus::{
            EventBus, EventResult,
            core_events::{
                BufferClosed, MotionContext, PluginTextInput, RequestCursorMove,
            },
        },
        keys,
        modd::ComponentId,
        plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
        subscribe_state, subscribe_state_conditional,
    },
};

// Re-export jump types
pub use jump::{
    Direction, JumpCancel, JumpExecute, JumpFindChar, JumpFindCharBack, JumpFindCharStarted,
    JumpInputChar, JumpMode, JumpSearch, JumpSearchStarted, JumpSelectLabel, JumpTillChar,
    JumpTillCharBack, OperatorContext, OperatorType, SharedJumpState,
};

// Re-export fold types
pub use fold::{
    FoldClose, FoldCloseAll, FoldOpen, FoldOpenAll, FoldRangesUpdated, FoldRenderStage, FoldToggle,
    SharedFoldManager,
};

/// Plugin-local command IDs
pub mod command_id {
    use super::CommandId;

    // Jump commands
    pub const JUMP_SEARCH: CommandId = CommandId::new("jump_search");
    pub const JUMP_FIND_CHAR: CommandId = CommandId::new("jump_find_char");
    pub const JUMP_FIND_CHAR_BACK: CommandId = CommandId::new("jump_find_char_back");
    pub const JUMP_TILL_CHAR: CommandId = CommandId::new("jump_till_char");
    pub const JUMP_TILL_CHAR_BACK: CommandId = CommandId::new("jump_till_char_back");
    pub const JUMP_CANCEL: CommandId = CommandId::new("jump_cancel");

    // Fold commands
    pub const FOLD_TOGGLE: CommandId = CommandId::new("fold_toggle");
    pub const FOLD_OPEN: CommandId = CommandId::new("fold_open");
    pub const FOLD_CLOSE: CommandId = CommandId::new("fold_close");
    pub const FOLD_OPEN_ALL: CommandId = CommandId::new("fold_open_all");
    pub const FOLD_CLOSE_ALL: CommandId = CommandId::new("fold_close_all");
}

/// Component ID for jump navigation interactor
pub const JUMP_COMPONENT_ID: ComponentId = ComponentId("range-finder-jump");

/// Unified range-finder plugin combining jump navigation and code folding
pub struct RangeFinderPlugin {
    jump_state: Arc<SharedJumpState>,
    fold_manager: Arc<SharedFoldManager>,
}

impl RangeFinderPlugin {
    /// Create a new `RangeFinderPlugin` with default configuration
    #[must_use]
    pub fn new() -> Self {
        Self {
            jump_state: Arc::new(SharedJumpState::new()),
            fold_manager: Arc::new(SharedFoldManager::new()),
        }
    }
}

impl Default for RangeFinderPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for RangeFinderPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("range-finder")
    }

    fn name(&self) -> &'static str {
        "Range-Finder"
    }

    fn description(&self) -> &'static str {
        "Unified jump navigation and code folding"
    }

    #[allow(clippy::too_many_lines)]
    fn build(&self, ctx: &mut PluginContext) {
        // === Register Display Info ===
        use reovim_core::{
            display::DisplayInfo,
            highlight::{Color, Style},
        };

        // Yellow/gold background for jump navigation (targeting/finding)
        let gold = Color::Rgb {
            r: 241,
            g: 196,
            b: 15,
        };
        let fg = Color::Rgb {
            r: 33,
            g: 33,
            b: 33,
        };
        let style = Style::new().fg(fg).bg(gold).bold();

        ctx.register_display(JUMP_COMPONENT_ID, DisplayInfo::new(" JUMP ", " ", style));

        // === Register Jump Commands ===
        let _ = ctx.register_command(JumpSearch::default_instance());
        let _ = ctx.register_command(JumpFindChar::default_instance());
        let _ = ctx.register_command(JumpFindCharBack::default_instance());
        let _ = ctx.register_command(JumpTillChar::default_instance());
        let _ = ctx.register_command(JumpTillCharBack::default_instance());
        let _ = ctx.register_command(JumpCancel);

        // === Register Fold Commands ===
        let _ = ctx.register_command(FoldToggle::default_instance());
        let _ = ctx.register_command(FoldOpen::default_instance());
        let _ = ctx.register_command(FoldClose::default_instance());
        let _ = ctx.register_command(FoldOpenAll::default_instance());
        let _ = ctx.register_command(FoldCloseAll::default_instance());

        // === Register Jump Keybindings (Normal mode + Operator-Pending) ===
        let normal_mode = KeymapScope::editor_normal();
        let operator_pending = KeymapScope::SubMode(SubModeKind::OperatorPending);

        // Multi-char search (works in both normal and operator-pending)
        ctx.bind_key_scoped(
            normal_mode.clone(),
            keys!['s'],
            CommandRef::Registered(command_id::JUMP_SEARCH),
        );
        ctx.bind_key_scoped(
            operator_pending.clone(),
            keys!['s'],
            CommandRef::Registered(command_id::JUMP_SEARCH),
        );

        // Enhanced f/t motions (works in both normal and operator-pending)
        ctx.bind_key_scoped(
            normal_mode.clone(),
            keys!['f'],
            CommandRef::Registered(command_id::JUMP_FIND_CHAR),
        );
        ctx.bind_key_scoped(
            operator_pending.clone(),
            keys!['f'],
            CommandRef::Registered(command_id::JUMP_FIND_CHAR),
        );

        ctx.bind_key_scoped(
            normal_mode.clone(),
            keys!['F'],
            CommandRef::Registered(command_id::JUMP_FIND_CHAR_BACK),
        );
        ctx.bind_key_scoped(
            operator_pending.clone(),
            keys!['F'],
            CommandRef::Registered(command_id::JUMP_FIND_CHAR_BACK),
        );

        ctx.bind_key_scoped(
            normal_mode.clone(),
            keys!['t'],
            CommandRef::Registered(command_id::JUMP_TILL_CHAR),
        );
        ctx.bind_key_scoped(
            operator_pending.clone(),
            keys!['t'],
            CommandRef::Registered(command_id::JUMP_TILL_CHAR),
        );

        ctx.bind_key_scoped(
            normal_mode.clone(),
            keys!['T'],
            CommandRef::Registered(command_id::JUMP_TILL_CHAR_BACK),
        );
        ctx.bind_key_scoped(
            operator_pending,
            keys!['T'],
            CommandRef::Registered(command_id::JUMP_TILL_CHAR_BACK),
        );

        // === Register Fold Keybindings (Normal mode) ===
        ctx.bind_key_scoped(
            normal_mode.clone(),
            keys!['z' 'a'],
            CommandRef::Registered(command_id::FOLD_TOGGLE),
        );

        ctx.bind_key_scoped(
            normal_mode.clone(),
            keys!['z' 'o'],
            CommandRef::Registered(command_id::FOLD_OPEN),
        );

        ctx.bind_key_scoped(
            normal_mode.clone(),
            keys!['z' 'c'],
            CommandRef::Registered(command_id::FOLD_CLOSE),
        );

        ctx.bind_key_scoped(
            normal_mode.clone(),
            keys!['z' 'R'],
            CommandRef::Registered(command_id::FOLD_OPEN_ALL),
        );

        ctx.bind_key_scoped(
            normal_mode,
            keys!['z' 'M'],
            CommandRef::Registered(command_id::FOLD_CLOSE_ALL),
        );

        // === Register Jump Cancel (Interactor mode - for when jump is active) ===
        // Note: Escape in Interactor mode will be handled by PluginTextInput routing

        tracing::info!("RangeFinderPlugin initialized with jump and fold subsystems");
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Register jump state
        registry.register(Arc::clone(&self.jump_state));

        // Register jump label window
        registry.register_plugin_window(Arc::new(jump::render::JumpLabelWindow));

        // Register fold manager
        registry.register(Arc::clone(&self.fold_manager));

        // Set fold manager as visibility source (for hiding folded lines)
        registry.set_visibility_source(Arc::clone(&self.fold_manager)
            as Arc<dyn reovim_core::visibility::BufferVisibilitySource>);

        // Register fold render stage
        registry
            .register_render_stage(Arc::new(FoldRenderStage::new(Arc::clone(&self.fold_manager))));
    }

    #[allow(clippy::too_many_lines)] // Unified subscription for jump + fold subsystems
    fn subscribe(&self, bus: &EventBus, _state: Arc<PluginStateRegistry>) {
        // === Jump Event Subscriptions ===

        // JumpSearchStarted - enter Interactor mode for multi-char search
        let jump_state = Arc::clone(&self.jump_state);
        bus.subscribe::<JumpSearchStarted, _>(100, move |event, ctx| {
            // Get operator context from event (set by command handler)
            let operator_context = event.operator_context;

            // Initialize jump state for multi-char search
            jump_state.with_mut(|state| {
                state.start_multi_char(
                    event.buffer_id,
                    event.cursor_line,
                    event.cursor_col,
                    event.direction,
                    event.lines.clone(),
                    operator_context,
                );
            });

            // Enter Interactor mode to receive character input
            ctx.enter_interactor_mode(JUMP_COMPONENT_ID);
            ctx.request_render();

            tracing::debug!(
                "Jump multi-char search started, operator_context={:?}",
                operator_context
            );
            EventResult::Handled
        });

        // JumpFindCharStarted - enter Interactor mode for single-char find/till
        let jump_state = Arc::clone(&self.jump_state);
        bus.subscribe::<JumpFindCharStarted, _>(100, move |event, ctx| {
            // Get operator context from event (set by command handler)
            let operator_context = event.operator_context;

            // Initialize jump state for single-char search
            jump_state.with_mut(|state| {
                state.start_find_char(
                    event.buffer_id,
                    event.cursor_line,
                    event.cursor_col,
                    event.direction,
                    event.lines.clone(),
                    operator_context,
                );
            });

            // Enter Interactor mode to receive character input
            ctx.enter_interactor_mode(JUMP_COMPONENT_ID);
            ctx.request_render();

            tracing::debug!(
                "Jump single-char search started (mode: {:?}), operator_context={:?}",
                event.mode,
                operator_context
            );
            EventResult::Handled
        });

        // PluginTextInput - handle character input during jump mode
        let jump_state = Arc::clone(&self.jump_state);
        bus.subscribe::<PluginTextInput, _>(100, move |event, ctx| {
            // Only handle if we're the target
            if event.target != JUMP_COMPONENT_ID {
                return EventResult::NotHandled;
            }

            // Handle input through the state machine
            let result = jump_state.with_mut(|state| {
                let buffer_id = state.buffer_id.unwrap_or(0);
                let lines = state.lines.clone().unwrap_or_default();
                let input_result = handle_char_input(state, event.c, &lines);
                tracing::debug!("Plugin handler: InputResult = {:?}", input_result);
                (buffer_id, input_result)
            });

            match result.1 {
                InputResult::Continue | InputResult::ShowLabels => {
                    tracing::debug!(
                        "Plugin handler: Calling request_render for Continue/ShowLabels"
                    );
                    ctx.request_render();
                    EventResult::Handled
                }
                InputResult::Jump(jump_execute) => {
                    tracing::debug!("Jump execute: {:?}", jump_execute);

                    // Get operator context before resetting state
                    let operator_context = jump_state.with_mut(|state| state.operator_context);

                    if let Some(op_ctx) = operator_context {
                        // Jump with operator - pass operator via motion_context
                        tracing::debug!("Jump with operator: {:?}", op_ctx);

                        // Pass operator directly in motion_context to avoid race condition
                        ctx.emit(RequestCursorMove {
                            buffer_id: jump_execute.buffer_id,
                            line: jump_execute.line,
                            column: jump_execute.col,
                            motion_context: Some(MotionContext {
                                linewise: false,
                                inclusive: true,
                                operator: Some(op_ctx.operator.into()),
                                count: op_ctx.count,
                            }),
                        });
                    } else {
                        // Normal jump without operator
                        ctx.emit(RequestCursorMove {
                            buffer_id: jump_execute.buffer_id,
                            line: jump_execute.line,
                            column: jump_execute.col,
                            motion_context: None,
                        });
                    }

                    // Return to Normal mode after jump
                    ctx.exit_to_normal();

                    // Reset jump state
                    jump_state.with_mut(jump::state::JumpState::reset);
                    ctx.request_render();

                    EventResult::Handled
                }
                InputResult::Cancel(reason) => {
                    tracing::debug!("Jump canceled: {:?}", reason);

                    // Reset state and exit Interactor mode
                    jump_state.with_mut(jump::state::JumpState::reset);
                    ctx.exit_to_normal();
                    ctx.request_render();

                    EventResult::Handled
                }
            }
        });

        // JumpCancel - handle explicit cancel (Escape key)
        let jump_state = Arc::clone(&self.jump_state);
        bus.subscribe::<JumpCancel, _>(100, move |_event, ctx| {
            // Reset jump state
            jump_state.with_mut(jump::state::JumpState::reset);

            // Exit Interactor mode
            ctx.exit_to_normal();
            ctx.request_render();

            tracing::debug!("Jump explicitly canceled");
            EventResult::Handled
        });

        // === Fold Event Subscriptions ===

        // Conditional render subscriptions (render only if state changed)
        subscribe_state_conditional!(bus, self.fold_manager, FoldToggle, |m, e| {
            m.toggle(e.buffer_id, e.line)
        });

        subscribe_state_conditional!(bus, self.fold_manager, FoldOpen, |m, e| {
            m.open(e.buffer_id, e.line)
        });

        subscribe_state_conditional!(bus, self.fold_manager, FoldClose, |m, e| {
            m.close(e.buffer_id, e.line)
        });

        // Simple mutation + render subscriptions
        subscribe_state!(bus, self.fold_manager, FoldOpenAll, |m, e| {
            m.open_all(e.buffer_id);
        });

        subscribe_state!(bus, self.fold_manager, FoldCloseAll, |m, e| {
            m.close_all(e.buffer_id);
        });

        // FoldRangesUpdated (from treesitter) - uses priority 50, keep manual
        let fold_manager = Arc::clone(&self.fold_manager);
        bus.subscribe::<FoldRangesUpdated, _>(50, move |event, ctx| {
            fold_manager.with_mut(|m| m.set_ranges(event.buffer_id, event.ranges.clone()));
            ctx.request_render();
            EventResult::Handled
        });

        // === Cleanup Subscriptions ===

        // BufferClosed - cleanup state
        let fold_manager = Arc::clone(&self.fold_manager);
        bus.subscribe::<BufferClosed, _>(100, move |event, _ctx| {
            fold_manager.with_mut(|m| m.remove_buffer(event.buffer_id));
            EventResult::Handled
        });
    }
}
