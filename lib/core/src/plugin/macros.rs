//! Subscription helper macros for reducing plugin boilerplate
//!
//! These macros reduce repetitive code when subscribing to events in plugins.
//! They handle the common patterns of state mutation, rendering, and mode changes.
//!
//! # Example
//!
//! ```ignore
//! use reovim_core::subscribe_state;
//!
//! // Registry-based state with event access
//! subscribe_state!(bus, state, MyCursorDown, MyState, |s, e| {
//!     s.move_cursor(e.count as isize);
//! });
//!
//! // Registry-based state without event access
//! subscribe_state!(bus, state, MyRefresh, MyState, |s| {
//!     s.refresh();
//! });
//!
//! // Arc-based state (plugin-owned)
//! subscribe_state!(bus, self.my_state, MyEvent, |s| {
//!     s.update();
//! });
//! ```

/// Subscribe to an event with automatic state mutation and render
///
/// This macro handles the common pattern of:
/// 1. Cloning the state Arc
/// 2. Subscribing to an event with plugin priority
/// 3. Mutating state via `with_mut`
/// 4. Requesting render
/// 5. Returning `EventResult::Handled`
///
/// # Variants
///
/// ## Registry-based state (`PluginStateRegistry`)
///
/// Without event access:
/// ```ignore
/// subscribe_state!(bus, state, EventType, StateType, |s| {
///     s.method();
/// });
/// ```
///
/// With event access:
/// ```ignore
/// subscribe_state!(bus, state, EventType, StateType, |s, e| {
///     s.method(e.count);
/// });
/// ```
///
/// ## Arc-based state (plugin-owned)
///
/// Without event access:
/// ```ignore
/// subscribe_state!(bus, arc_state, EventType, |s| {
///     s.method();
/// });
/// ```
///
/// With event access:
/// ```ignore
/// subscribe_state!(bus, arc_state, EventType, |s, e| {
///     s.method(e.field);
/// });
/// ```
#[macro_export]
macro_rules! subscribe_state {
    // Registry-based, no event access
    ($bus:expr, $state:expr, $event:ty, $state_type:ty, |$s:ident| $body:block) => {{
        let state_clone = std::sync::Arc::clone(&$state);
        $bus.subscribe::<$event, _>($crate::event_bus::priority::PLUGIN, move |_event, ctx| {
            state_clone.with_mut::<$state_type, _, _>(|$s| $body);
            ctx.request_render();
            $crate::event_bus::EventResult::Handled
        });
    }};

    // Registry-based, with event access
    ($bus:expr, $state:expr, $event:ty, $state_type:ty, |$s:ident, $e:ident| $body:block) => {{
        let state_clone = std::sync::Arc::clone(&$state);
        $bus.subscribe::<$event, _>($crate::event_bus::priority::PLUGIN, move |$e, ctx| {
            state_clone.with_mut::<$state_type, _, _>(|$s| $body);
            ctx.request_render();
            $crate::event_bus::EventResult::Handled
        });
    }};

    // Arc-based (plugin-owned state), no event access
    ($bus:expr, $arc_state:expr, $event:ty, |$s:ident| $body:block) => {{
        let state_clone = std::sync::Arc::clone(&$arc_state);
        $bus.subscribe::<$event, _>($crate::event_bus::priority::PLUGIN, move |_event, ctx| {
            state_clone.with_mut(|$s| $body);
            ctx.request_render();
            $crate::event_bus::EventResult::Handled
        });
    }};

    // Arc-based (plugin-owned state), with event access
    ($bus:expr, $arc_state:expr, $event:ty, |$s:ident, $e:ident| $body:block) => {{
        let state_clone = std::sync::Arc::clone(&$arc_state);
        $bus.subscribe::<$event, _>($crate::event_bus::priority::PLUGIN, move |$e, ctx| {
            state_clone.with_mut(|$s| $body);
            ctx.request_render();
            $crate::event_bus::EventResult::Handled
        });
    }};
}

/// Subscribe to an event with state mutation, mode change, and render
///
/// This macro handles the pattern of:
/// 1. Cloning the state Arc
/// 2. Subscribing to an event with plugin priority
/// 3. Mutating state via `with_mut`
/// 4. Emitting `RequestModeChange` with the provided mode
/// 5. Requesting render
/// 6. Returning `EventResult::Handled`
///
/// # Example
///
/// ```ignore
/// subscribe_state_mode!(
///     bus, state, ExplorerCreateFile, ExplorerState,
///     |s| { s.start_create_file(); },
///     ModeState::with_interactor_id_sub_mode(
///         COMPONENT_ID,
///         EditMode::Normal,
///         SubMode::Interactor(COMPONENT_ID),
///     )
/// );
/// ```
#[macro_export]
macro_rules! subscribe_state_mode {
    // Registry-based
    ($bus:expr, $state:expr, $event:ty, $state_type:ty, |$s:ident| $body:block, $mode:expr) => {{
        let state_clone = std::sync::Arc::clone(&$state);
        // Evaluate mode expression before closure to avoid move issues
        let mode = $mode;
        $bus.subscribe::<$event, _>($crate::event_bus::priority::PLUGIN, move |_event, ctx| {
            state_clone.with_mut::<$state_type, _, _>(|$s| $body);
            ctx.emit($crate::event_bus::RequestModeChange { mode: mode.clone() });
            ctx.request_render();
            $crate::event_bus::EventResult::Handled
        });
    }};

    // Arc-based
    ($bus:expr, $arc_state:expr, $event:ty, |$s:ident| $body:block, $mode:expr) => {{
        let state_clone = std::sync::Arc::clone(&$arc_state);
        // Evaluate mode expression before closure to avoid move issues
        let mode = $mode;
        $bus.subscribe::<$event, _>($crate::event_bus::priority::PLUGIN, move |_event, ctx| {
            state_clone.with_mut(|$s| $body);
            ctx.emit($crate::event_bus::RequestModeChange { mode: mode.clone() });
            ctx.request_render();
            $crate::event_bus::EventResult::Handled
        });
    }};
}

/// Subscribe with conditional render based on return value
///
/// This macro handles the pattern where rendering should only occur
/// if the state mutation returns `true` (indicating a change occurred).
///
/// # Example
///
/// ```ignore
/// subscribe_state_conditional!(bus, self.fold_manager, FoldToggle, |m, e| {
///     m.toggle(e.buffer_id, e.line)
/// });
/// ```
#[macro_export]
macro_rules! subscribe_state_conditional {
    // Arc-based with event access (most common for fold operations)
    ($bus:expr, $arc_state:expr, $event:ty, |$s:ident, $e:ident| $body:expr) => {{
        let state_clone = std::sync::Arc::clone(&$arc_state);
        $bus.subscribe::<$event, _>($crate::event_bus::priority::PLUGIN, move |$e, ctx| {
            let changed = state_clone.with_mut(|$s| $body);
            if changed {
                ctx.request_render();
            }
            $crate::event_bus::EventResult::Handled
        });
    }};

    // Arc-based without event access
    ($bus:expr, $arc_state:expr, $event:ty, |$s:ident| $body:expr) => {{
        let state_clone = std::sync::Arc::clone(&$arc_state);
        $bus.subscribe::<$event, _>($crate::event_bus::priority::PLUGIN, move |_event, ctx| {
            let changed = state_clone.with_mut(|$s| $body);
            if changed {
                ctx.request_render();
            }
            $crate::event_bus::EventResult::Handled
        });
    }};

    // Registry-based with event access
    ($bus:expr, $state:expr, $event:ty, $state_type:ty, |$s:ident, $e:ident| $body:expr) => {{
        let state_clone = std::sync::Arc::clone(&$state);
        $bus.subscribe::<$event, _>($crate::event_bus::priority::PLUGIN, move |$e, ctx| {
            let changed = state_clone
                .with_mut::<$state_type, _, _>(|$s| $body)
                .unwrap_or(false);
            if changed {
                ctx.request_render();
            }
            $crate::event_bus::EventResult::Handled
        });
    }};

    // Registry-based without event access
    ($bus:expr, $state:expr, $event:ty, $state_type:ty, |$s:ident| $body:expr) => {{
        let state_clone = std::sync::Arc::clone(&$state);
        $bus.subscribe::<$event, _>($crate::event_bus::priority::PLUGIN, move |_event, ctx| {
            let changed = state_clone
                .with_mut::<$state_type, _, _>(|$s| $body)
                .unwrap_or(false);
            if changed {
                ctx.request_render();
            }
            $crate::event_bus::EventResult::Handled
        });
    }};
}
