//! Command declaration macros
//!
//! These macros reduce boilerplate when declaring commands that emit events.

/// Declare a simple event-emitting command
///
/// This macro generates a zero-sized command type that emits a specified event
/// when executed. The event type must implement `Default`.
///
/// # Examples
///
/// ```
/// use reovim_core::declare_command;
/// use reovim_core::event_bus::Event;
///
/// #[derive(Debug, Clone, Copy, Default)]
/// struct MyEvent;
/// impl Event for MyEvent {}
///
/// declare_command! {
///     MyCommand => MyEvent,
///     name: "my_command",
///     description: "Does something useful",
/// }
/// ```
#[macro_export]
macro_rules! declare_command {
    (
        $cmd_name:ident => $event_type:ty,
        name: $name:expr,
        description: $desc:expr $(,)?
    ) => {
        #[derive(Debug, Clone, Copy)]
        pub struct $cmd_name;

        impl $crate::command::CommandTrait for $cmd_name {
            fn name(&self) -> &'static str {
                $name
            }

            fn description(&self) -> &'static str {
                $desc
            }

            fn execute(
                &self,
                _ctx: &mut $crate::command::ExecutionContext,
            ) -> $crate::command::CommandResult {
                $crate::command::CommandResult::EmitEvent($crate::event_bus::DynEvent::new(
                    <$event_type>::default(),
                ))
            }

            fn clone_box(&self) -> Box<dyn $crate::command::CommandTrait> {
                Box::new(*self)
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }
    };
}

/// Declare a command that takes a count parameter
///
/// This macro generates a command type that extracts the count from the execution
/// context and passes it to the event constructor. The event type must have a
/// `new(usize)` constructor.
///
/// # Examples
///
/// ```
/// use reovim_core::declare_counted_command;
/// use reovim_core::event_bus::Event;
///
/// #[derive(Debug, Clone)]
/// struct MoveEvent {
///     count: usize,
/// }
///
/// impl MoveEvent {
///     fn new(count: usize) -> Self {
///         Self { count }
///     }
/// }
///
/// impl Event for MoveEvent {}
///
/// declare_counted_command! {
///     MoveCommand => MoveEvent,
///     name: "move",
///     description: "Move by count",
/// }
/// ```
#[macro_export]
macro_rules! declare_counted_command {
    (
        $cmd_name:ident => $event_type:ty,
        name: $name:expr,
        description: $desc:expr $(,)?
    ) => {
        #[derive(Debug, Clone, Copy)]
        pub struct $cmd_name;

        impl $crate::command::CommandTrait for $cmd_name {
            fn name(&self) -> &'static str {
                $name
            }

            fn description(&self) -> &'static str {
                $desc
            }

            fn execute(
                &self,
                ctx: &mut $crate::command::ExecutionContext,
            ) -> $crate::command::CommandResult {
                let count = ctx.count.unwrap_or(1);
                $crate::command::CommandResult::EmitEvent($crate::event_bus::DynEvent::new(
                    <$event_type>::new(count),
                ))
            }

            fn clone_box(&self) -> Box<dyn $crate::command::CommandTrait> {
                Box::new(*self)
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }
    };
}

/// Declare a command that is also an event (unified type)
///
/// This macro generates a single type that implements both `CommandTrait` and `Event`.
/// This eliminates the need for separate Command and Event types for simple actions.
///
/// The generated type will be zero-sized and implement `Default`, `Clone`, `Copy`.
///
/// # Examples
///
/// ```
/// use reovim_core::declare_event_command;
///
/// declare_event_command! {
///     CursorUp,
///     id: "cursor_up",
///     description: "Move cursor up",
/// }
///
/// // Can be used as both command and event:
/// // ctx.register_command(CursorUp);
/// // bus.subscribe::<CursorUp, _>(100, |event, ctx| { ... });
/// ```
#[macro_export]
macro_rules! declare_event_command {
    (
        $name:ident,
        id: $id:expr,
        description: $desc:expr $(,)?
    ) => {
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        impl $crate::command::CommandTrait for $name {
            fn name(&self) -> &'static str {
                $id
            }

            fn description(&self) -> &'static str {
                $desc
            }

            fn execute(
                &self,
                _ctx: &mut $crate::command::ExecutionContext,
            ) -> $crate::command::CommandResult {
                $crate::command::CommandResult::EmitEvent($crate::event_bus::DynEvent::new(*self))
            }

            fn clone_box(&self) -> Box<dyn $crate::command::CommandTrait> {
                Box::new(*self)
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        impl $crate::event_bus::Event for $name {
            fn priority(&self) -> u32 {
                $crate::event_bus::priority::NORMAL
            }
        }
    };
}

/// Declare a counted event-command (unified type with count parameter)
///
/// This macro generates a single type that implements both `CommandTrait` and `Event`,
/// and carries a count parameter.
///
/// # Examples
///
/// ```
/// use reovim_core::declare_counted_event_command;
///
/// declare_counted_event_command! {
///     CursorDown,
///     id: "cursor_down",
///     description: "Move cursor down by count",
/// }
///
/// // Used as both command and event, with count support
/// // ctx.register_command(CursorDown);
/// // bus.subscribe::<CursorDown, _>(100, |event, ctx| {
/// //     let count = event.count;
/// // });
/// ```
#[macro_export]
macro_rules! declare_counted_event_command {
    (
        $name:ident,
        id: $id:expr,
        description: $desc:expr $(,)?
    ) => {
        #[derive(Debug, Clone, Copy)]
        pub struct $name {
            pub count: usize,
        }

        impl $name {
            pub fn new(count: usize) -> Self {
                Self { count }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self { count: 1 }
            }
        }

        impl $crate::command::CommandTrait for $name {
            fn name(&self) -> &'static str {
                $id
            }

            fn description(&self) -> &'static str {
                $desc
            }

            fn execute(
                &self,
                ctx: &mut $crate::command::ExecutionContext,
            ) -> $crate::command::CommandResult {
                let count = ctx.count.unwrap_or(1);
                $crate::command::CommandResult::EmitEvent($crate::event_bus::DynEvent::new(
                    Self::new(count),
                ))
            }

            fn clone_box(&self) -> Box<dyn $crate::command::CommandTrait> {
                Box::new(*self)
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        impl $crate::event_bus::Event for $name {
            fn priority(&self) -> u32 {
                $crate::event_bus::priority::NORMAL
            }
        }
    };
}
