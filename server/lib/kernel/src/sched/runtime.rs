//! Runtime event loop coordinator.
//!
//! Linux equivalent: Main scheduler in `kernel/sched/core.c`
//!
//! The [`Runtime`] is the central coordinator for event processing and task
//! execution. It manages the event loop lifecycle, processes events from the
//! priority queue, and executes deferred tasks.
//!
//! # Design Philosophy
//!
//! - **Pure coordination**: Runtime doesn't own business logic, just orchestrates
//! - **Sync execution**: All processing happens synchronously (async is driver responsibility)
//! - **Panic safety**: Tasks that panic are caught and don't crash the runtime
//! - **Graceful shutdown**: Supports clean shutdown with work draining
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::*;
//!
//! let mut runtime = Runtime::new();
//! runtime.boot();
//!
//! // Schedule some work
//! runtime.schedule_work(|| println!("Deferred work!"));
//!
//! // Process pending work and check if idle
//! runtime.tick();
//! assert!(runtime.is_idle());
//!
//! // Request shutdown
//! runtime.shutdown();
//! assert_eq!(runtime.state(), RuntimeState::Stopping);
//! ```

use std::{sync::Arc, time::Duration};

use {
    super::{
        executor::Executor,
        priority::PriorityQueue,
        state::RuntimeState,
        task::{Priority, Task},
        timer::{DEFAULT_MAX_TIMERS, TimerHandle, TimerId, TimerWheel},
        work_queue::WorkQueue,
    },
    crate::ipc::{DynEvent, EventBus, EventScope, Receiver, Sender, channel},
};

/// Default work queue capacity.
pub const DEFAULT_WORK_QUEUE_CAPACITY: usize = 1024;

/// Default priority queue capacity.
pub const DEFAULT_PRIORITY_QUEUE_CAPACITY: usize = 256;

/// Default batch size for task execution.
pub const DEFAULT_BATCH_SIZE: usize = 16;

/// Runtime configuration.
///
/// Used with [`Runtime::with_config()`] to customize runtime behavior.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// let config = RuntimeConfig {
///     work_queue_capacity: 2048,
///     priority_queue_capacity: 512,
///     batch_size: 32,
///     max_timers: 512,
/// };
///
/// let runtime = Runtime::with_config(config);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct RuntimeConfig {
    /// Maximum tasks in the work queue.
    pub work_queue_capacity: usize,

    /// Maximum events in the priority queue.
    pub priority_queue_capacity: usize,

    /// Maximum tasks processed per tick.
    pub batch_size: usize,

    /// Maximum concurrent timers.
    pub max_timers: usize,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            work_queue_capacity: DEFAULT_WORK_QUEUE_CAPACITY,
            priority_queue_capacity: DEFAULT_PRIORITY_QUEUE_CAPACITY,
            batch_size: DEFAULT_BATCH_SIZE,
            max_timers: DEFAULT_MAX_TIMERS,
        }
    }
}

/// Commands that can be sent to the runtime.
///
/// Used for external control of the runtime lifecycle.
#[derive(Debug)]
pub enum RuntimeCommand {
    /// Request graceful shutdown.
    Shutdown,

    /// Request emergency stop (panic recovery).
    Emergency,

    /// Schedule a task for execution.
    ScheduleTask(Task),
}

/// Runtime execution statistics.
///
/// Provides insight into runtime operation for monitoring and debugging.
#[derive(Debug, Clone, Copy, Default)]
pub struct RuntimeStats {
    /// Current runtime state.
    pub state: RuntimeState,

    /// Number of events in the priority queue.
    pub event_queue_len: usize,

    /// Number of tasks in the work queue.
    pub work_queue_len: usize,

    /// Number of tasks dropped due to overflow.
    pub work_dropped: usize,

    /// Total tasks successfully executed.
    pub tasks_executed: u64,

    /// Total tasks that failed (error or panic).
    pub tasks_failed: u64,

    /// Number of active (pending) timers.
    pub active_timers: usize,

    /// Number of timers dropped due to capacity limit.
    pub timers_dropped: u64,
}

/// Runtime event loop coordinator.
///
/// The `Runtime` manages the main event loop, coordinating between:
/// - Event processing (via [`PriorityQueue`] and [`EventBus`])
/// - Task execution (via [`WorkQueue`] and [`Executor`])
/// - Lifecycle management (boot, run, shutdown)
///
/// # Thread Safety
///
/// The runtime itself is `Send` but not `Sync` - it should be owned by a
/// single thread that drives the event loop. However, the command channel
/// and work queue can be accessed from other threads for scheduling.
///
/// # State Transitions
///
/// ```text
/// Booting → Running → Stopping
///    ↓         ↓          ↓
///    └─────────┴──────────┴→ Emergency
/// ```
pub struct Runtime {
    /// Current lifecycle state.
    state: RuntimeState,

    /// Event bus for pub/sub communication.
    event_bus: Arc<EventBus>,

    /// Priority queue for ordered event processing.
    event_queue: PriorityQueue,

    /// Work queue for deferred tasks.
    work_queue: Arc<WorkQueue>,

    /// Timer wheel for delayed/periodic work.
    timer_wheel: Arc<TimerWheel>,

    /// Task executor with panic handling.
    executor: Executor,

    /// Sender for runtime commands.
    command_tx: Sender<RuntimeCommand>,

    /// Receiver for runtime commands.
    command_rx: Receiver<RuntimeCommand>,

    /// Whether a render is pending.
    render_pending: bool,

    /// Current event scope for lifecycle tracking.
    current_scope: Option<EventScope>,
}

impl Runtime {
    /// Create a new runtime with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(RuntimeConfig::default())
    }

    /// Create a runtime with custom configuration.
    #[must_use]
    pub fn with_config(config: RuntimeConfig) -> Self {
        let work_queue = Arc::new(WorkQueue::with_capacity(config.work_queue_capacity));
        let executor = Executor::new(Arc::clone(&work_queue)).with_batch_size(config.batch_size);
        let event_queue = PriorityQueue::with_capacity(config.priority_queue_capacity);
        let timer_wheel = Arc::new(TimerWheel::with_max_timers(config.max_timers));
        let (command_tx, command_rx) = channel();

        Self {
            state: RuntimeState::Booting,
            event_bus: Arc::new(EventBus::new()),
            event_queue,
            work_queue,
            timer_wheel,
            executor,
            command_tx,
            command_rx,
            render_pending: false,
            current_scope: None,
        }
    }

    /// Create a runtime with a shared event bus.
    ///
    /// This is useful when the event bus is shared with other subsystems.
    #[must_use]
    pub fn with_event_bus(mut self, event_bus: Arc<EventBus>) -> Self {
        self.event_bus = event_bus;
        self
    }

    /// Boot the runtime, transitioning from `Booting` to `Running`.
    ///
    /// # Panics
    ///
    /// Panics if called when not in `Booting` state.
    pub fn boot(&mut self) {
        assert_eq!(self.state, RuntimeState::Booting, "boot() called when not in Booting state");
        self.state = RuntimeState::Running;
    }

    /// Get the current runtime state.
    #[inline]
    #[must_use]
    pub const fn state(&self) -> RuntimeState {
        self.state
    }

    /// Get a reference to the event bus.
    #[inline]
    #[must_use]
    pub const fn event_bus(&self) -> &Arc<EventBus> {
        &self.event_bus
    }

    /// Get a clone of the command sender.
    ///
    /// The sender can be used from other threads to send commands.
    #[must_use]
    pub fn command_sender(&self) -> Sender<RuntimeCommand> {
        self.command_tx.clone()
    }

    /// Process one tick of the event loop.
    ///
    /// This method:
    /// 1. Processes runtime commands
    /// 2. Processes expired timers (schedules their callbacks as tasks)
    /// 3. Dispatches events from the priority queue
    /// 4. Executes tasks from the work queue
    /// 5. Processes queued async events
    ///
    /// Returns `true` if the runtime should continue, `false` if it should stop.
    pub fn tick(&mut self) -> bool {
        // Process commands first
        self.process_commands();

        // Check if we should stop
        if self.state.is_terminal() {
            // Drain remaining work before stopping (graceful shutdown)
            if self.state == RuntimeState::Stopping {
                self.executor.drain();
            }
            return false;
        }

        // Only process work when running
        if self.state.is_running() {
            // Process timers FIRST - may schedule new work
            let timer_tasks = self.timer_wheel.tick(std::time::Instant::now());
            for task in timer_tasks {
                self.work_queue.push(task);
            }

            // Dispatch events from priority queue
            self.dispatch_events();

            // Execute tasks from work queue
            self.executor.tick();

            // Process queued async events
            let _ = self.event_bus.process_queue();
        }

        true
    }

    /// Process pending runtime commands.
    fn process_commands(&mut self) {
        while let Ok(command) = self.command_rx.try_recv() {
            match command {
                RuntimeCommand::Shutdown => {
                    if self.state == RuntimeState::Running {
                        self.state = RuntimeState::Stopping;
                    }
                }
                RuntimeCommand::Emergency => {
                    self.state = RuntimeState::Emergency;
                }
                RuntimeCommand::ScheduleTask(task) => {
                    if self.state.can_accept_work() {
                        self.work_queue.push(task);
                    }
                }
            }
        }
    }

    /// Dispatch events from the priority queue to handlers.
    fn dispatch_events(&self) {
        while let Some(event) = self.event_queue.pop() {
            let _result = self.event_bus.dispatch(&event);
            // Decrement scope if present
            if let Some(ref scope) = self.current_scope
                && scope.in_flight() > 0
            {
                scope.decrement();
            }
        }
    }

    /// Schedule a closure for deferred execution.
    ///
    /// Returns `false` if the queue is full or runtime is not accepting work.
    pub fn schedule_work<F>(&self, work: F) -> bool
    where
        F: FnOnce() + Send + 'static,
    {
        if !self.state.can_accept_work() {
            return false;
        }
        self.work_queue.push(Task::new(work))
    }

    /// Schedule a closure with specific priority.
    ///
    /// Returns `false` if the queue is full or runtime is not accepting work.
    pub fn schedule_work_with_priority<F>(&self, priority: Priority, work: F) -> bool
    where
        F: FnOnce() + Send + 'static,
    {
        if !self.state.can_accept_work() {
            return false;
        }
        self.work_queue.push(Task::with_priority(priority, work))
    }

    /// Schedule a pre-built task for execution.
    ///
    /// Returns `false` if the queue is full or runtime is not accepting work.
    pub fn schedule_task(&self, task: Task) -> bool {
        if !self.state.can_accept_work() {
            return false;
        }
        self.work_queue.push(task)
    }

    /// Schedule work to execute after a delay (one-shot timer).
    ///
    /// The callback executes once after the delay passes, on the next tick
    /// after the deadline. Returns a handle that cancels the timer when dropped.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::api::v1::*;
    /// use std::time::Duration;
    ///
    /// let mut runtime = Runtime::new();
    /// runtime.boot();
    ///
    /// // Schedule work for 100ms from now
    /// let handle = runtime.schedule_delayed(Duration::from_millis(100), || {
    ///     println!("Delayed work executed!");
    /// });
    ///
    /// // Timer will fire on the tick after 100ms passes
    /// // Drop handle to cancel, or let it fire
    /// ```
    pub fn schedule_delayed<F>(&self, delay: Duration, work: F) -> TimerHandle
    where
        F: FnOnce() + Send + 'static,
    {
        self.timer_wheel
            .schedule_oneshot(delay, Priority::NORMAL, work)
    }

    /// Schedule work to execute periodically.
    ///
    /// The callback executes repeatedly at the specified interval. Returns a
    /// handle that cancels the timer when dropped.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::api::v1::*;
    /// use std::time::Duration;
    ///
    /// let mut runtime = Runtime::new();
    /// runtime.boot();
    ///
    /// // Schedule periodic work every 50ms
    /// let handle = runtime.schedule_periodic(Duration::from_millis(50), || {
    ///     println!("Periodic work!");
    /// });
    ///
    /// // Timer fires every 50ms until handle is dropped
    /// ```
    pub fn schedule_periodic<F>(&self, interval: Duration, work: F) -> TimerHandle
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.timer_wheel
            .schedule_periodic(interval, Priority::NORMAL, work)
    }

    /// Cancel a timer by ID.
    ///
    /// Returns `true` if the timer was found and cancelled, `false` if it
    /// was already cancelled or has already fired.
    ///
    /// Note: Timers are automatically cancelled when their [`TimerHandle`] is
    /// dropped, so explicit cancellation is rarely needed.
    pub fn cancel_timer(&self, id: TimerId) -> bool {
        self.timer_wheel.cancel(id)
    }

    /// Get a reference to the timer wheel.
    ///
    /// This allows external code to schedule timers directly with custom
    /// configuration.
    #[must_use]
    pub const fn timer_wheel(&self) -> &Arc<TimerWheel> {
        &self.timer_wheel
    }

    /// Queue an event for priority-ordered processing.
    ///
    /// Returns `false` if the queue is full.
    pub fn queue_event(&self, event: DynEvent) -> bool {
        self.event_queue.push(event)
    }

    /// Request a render on the next tick.
    pub const fn request_render(&mut self) {
        self.render_pending = true;
    }

    /// Check and clear the render pending flag.
    ///
    /// Returns `true` if a render was requested since the last call.
    pub const fn take_render_pending(&mut self) -> bool {
        let pending = self.render_pending;
        self.render_pending = false;
        pending
    }

    /// Check if a render is pending.
    #[inline]
    #[must_use]
    pub const fn is_render_pending(&self) -> bool {
        self.render_pending
    }

    /// Set the current event scope for lifecycle tracking.
    pub fn set_scope(&mut self, scope: EventScope) {
        self.current_scope = Some(scope);
    }

    /// Clear the current event scope.
    pub fn clear_scope(&mut self) {
        self.current_scope = None;
    }

    /// Get a reference to the current scope.
    #[must_use]
    pub const fn current_scope(&self) -> Option<&EventScope> {
        self.current_scope.as_ref()
    }

    /// Request graceful shutdown.
    ///
    /// The runtime will finish processing current work before stopping.
    pub fn shutdown(&mut self) {
        if self.state == RuntimeState::Running {
            self.state = RuntimeState::Stopping;
        }
    }

    /// Request emergency stop.
    ///
    /// The runtime will stop immediately without draining work.
    pub const fn emergency_stop(&mut self) {
        self.state = RuntimeState::Emergency;
    }

    /// Check if the runtime is idle (no pending work).
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.event_queue.is_empty()
            && self.work_queue.is_empty()
            && self.event_bus.queue_is_empty()
            && self.timer_wheel.pending_count() == 0
    }

    /// Get runtime statistics.
    #[must_use]
    pub fn stats(&self) -> RuntimeStats {
        RuntimeStats {
            state: self.state,
            event_queue_len: self.event_queue.len(),
            work_queue_len: self.work_queue.len(),
            work_dropped: self.work_queue.dropped_count(),
            tasks_executed: self.executor.executed_count(),
            tasks_failed: self.executor.failed_count(),
            active_timers: self.timer_wheel.pending_count(),
            timers_dropped: self.timer_wheel.dropped_count(),
        }
    }

    /// Get a reference to the work queue.
    ///
    /// This allows external code to schedule tasks directly.
    #[must_use]
    pub const fn work_queue(&self) -> &Arc<WorkQueue> {
        &self.work_queue
    }

    /// Run until the runtime stops.
    ///
    /// This is a convenience method that calls `tick()` in a loop.
    /// For more control, use `tick()` directly.
    pub fn run(&mut self) {
        while self.tick() {}
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime")
            .field("state", &self.state)
            .field("event_queue_len", &self.event_queue.len())
            .field("work_queue_len", &self.work_queue.len())
            .field("render_pending", &self.render_pending)
            .field("has_scope", &self.current_scope.is_some())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    // === Basic tests ===

    #[test]
    fn test_runtime_new() {
        let runtime = Runtime::new();
        assert_eq!(runtime.state(), RuntimeState::Booting);
        assert!(runtime.is_idle());
    }

    #[test]
    fn test_runtime_with_config() {
        let config = RuntimeConfig {
            work_queue_capacity: 100,
            priority_queue_capacity: 50,
            batch_size: 8,
            max_timers: 64,
        };
        let runtime = Runtime::with_config(config);
        assert_eq!(runtime.state(), RuntimeState::Booting);
    }

    #[test]
    fn test_runtime_boot() {
        let mut runtime = Runtime::new();
        assert_eq!(runtime.state(), RuntimeState::Booting);

        runtime.boot();
        assert_eq!(runtime.state(), RuntimeState::Running);
    }

    #[test]
    #[should_panic(expected = "boot() called when not in Booting state")]
    fn test_runtime_boot_twice_panics() {
        let mut runtime = Runtime::new();
        runtime.boot();
        runtime.boot(); // Should panic
    }

    // === Shutdown tests ===

    #[test]
    fn test_runtime_shutdown() {
        let mut runtime = Runtime::new();
        runtime.boot();
        assert_eq!(runtime.state(), RuntimeState::Running);

        runtime.shutdown();
        assert_eq!(runtime.state(), RuntimeState::Stopping);
    }

    #[test]
    fn test_runtime_emergency_stop() {
        let mut runtime = Runtime::new();
        runtime.boot();

        runtime.emergency_stop();
        assert_eq!(runtime.state(), RuntimeState::Emergency);
    }

    #[test]
    fn test_runtime_shutdown_via_command() {
        let mut runtime = Runtime::new();
        runtime.boot();

        let sender = runtime.command_sender();
        sender.send(RuntimeCommand::Shutdown).unwrap();

        runtime.tick();
        assert_eq!(runtime.state(), RuntimeState::Stopping);
    }

    // === Task scheduling tests ===

    #[test]
    fn test_runtime_schedule_work() {
        let mut runtime = Runtime::new();
        runtime.boot();

        let executed = Arc::new(AtomicBool::new(false));
        let executed_clone = Arc::clone(&executed);

        assert!(runtime.schedule_work(move || {
            executed_clone.store(true, Ordering::SeqCst);
        }));

        runtime.tick();
        assert!(executed.load(Ordering::SeqCst));
    }

    #[test]
    fn test_runtime_schedule_work_not_running() {
        let runtime = Runtime::new(); // Still booting
        assert!(!runtime.schedule_work(|| {}));
    }

    #[test]
    fn test_runtime_schedule_task() {
        let mut runtime = Runtime::new();
        runtime.boot();

        let executed = Arc::new(AtomicBool::new(false));
        let executed_clone = Arc::clone(&executed);

        let task = Task::new(move || {
            executed_clone.store(true, Ordering::SeqCst);
        });

        assert!(runtime.schedule_task(task));
        runtime.tick();
        assert!(executed.load(Ordering::SeqCst));
    }

    #[test]
    fn test_runtime_schedule_work_with_priority() {
        let mut runtime = Runtime::new();
        runtime.boot();

        let order = Arc::new(std::sync::Mutex::new(Vec::new()));

        let order1 = Arc::clone(&order);
        runtime.schedule_work_with_priority(Priority::LOW, move || {
            order1.lock().unwrap().push("low");
        });

        let order2 = Arc::clone(&order);
        runtime.schedule_work_with_priority(Priority::HIGH, move || {
            order2.lock().unwrap().push("high");
        });

        // Note: WorkQueue is FIFO, not priority-ordered
        // Priority only matters for tasks in PriorityQueue
        runtime.tick();
        runtime.tick();

        let result = order.lock().unwrap().clone();
        // Tasks are executed in FIFO order from WorkQueue
        assert_eq!(result.len(), 2);
    }

    // === Render pending tests ===

    #[test]
    fn test_runtime_render_pending() {
        let mut runtime = Runtime::new();

        assert!(!runtime.is_render_pending());

        runtime.request_render();
        assert!(runtime.is_render_pending());

        assert!(runtime.take_render_pending());
        assert!(!runtime.is_render_pending());
    }

    // === Scope tests ===

    #[test]
    fn test_runtime_scope() {
        let mut runtime = Runtime::new();
        assert!(runtime.current_scope().is_none());

        let scope = EventScope::new();
        runtime.set_scope(scope);
        assert!(runtime.current_scope().is_some());

        runtime.clear_scope();
        assert!(runtime.current_scope().is_none());
    }

    // === Idle tests ===

    #[test]
    fn test_runtime_is_idle() {
        let mut runtime = Runtime::new();
        runtime.boot();
        assert!(runtime.is_idle());

        runtime.schedule_work(|| {});
        assert!(!runtime.is_idle());

        runtime.tick();
        assert!(runtime.is_idle());
    }

    // === Stats tests ===

    #[test]
    fn test_runtime_stats() {
        let mut runtime = Runtime::new();
        runtime.boot();

        runtime.schedule_work(|| {});
        runtime.schedule_work(|| panic!("intentional panic"));

        let stats = runtime.stats();
        assert_eq!(stats.state, RuntimeState::Running);
        assert_eq!(stats.work_queue_len, 2);
        assert_eq!(stats.tasks_executed, 0);

        runtime.tick();

        let stats = runtime.stats();
        assert_eq!(stats.work_queue_len, 0);
        // One succeeded, one failed
        assert_eq!(stats.tasks_executed, 1);
        assert_eq!(stats.tasks_failed, 1);
    }

    // === Tick behavior tests ===

    #[test]
    fn test_runtime_tick_returns_false_on_stop() {
        let mut runtime = Runtime::new();
        runtime.boot();

        assert!(runtime.tick());

        runtime.shutdown();
        assert!(!runtime.tick());
    }

    #[test]
    fn test_runtime_tick_drains_on_shutdown() {
        let mut runtime = Runtime::new();
        runtime.boot();

        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..5 {
            let counter_clone = Arc::clone(&counter);
            runtime.schedule_work(move || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            });
        }

        runtime.shutdown();
        runtime.tick(); // Should drain all tasks

        assert_eq!(counter.load(Ordering::SeqCst), 5);
    }

    // === Run tests ===

    #[test]
    fn test_runtime_run() {
        let mut runtime = Runtime::new();
        runtime.boot();

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        // Schedule work that will trigger shutdown
        runtime.schedule_work(move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Send shutdown command
        let sender = runtime.command_sender();
        sender.send(RuntimeCommand::Shutdown).unwrap();

        runtime.run();

        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert!(runtime.state().is_shutting_down());
    }

    // === Debug tests ===

    #[test]
    fn test_runtime_debug() {
        let runtime = Runtime::new();
        let debug_str = format!("{runtime:?}");
        assert!(debug_str.contains("Runtime"));
        assert!(debug_str.contains("Booting"));
    }

    // === Event bus integration ===

    #[test]
    fn test_runtime_with_event_bus() {
        let bus = Arc::new(EventBus::new());
        let bus_clone = Arc::clone(&bus);

        let runtime = Runtime::new().with_event_bus(bus);

        // Both should point to the same EventBus
        assert!(Arc::ptr_eq(runtime.event_bus(), &bus_clone));
    }

    // === Command channel tests ===

    #[test]
    fn test_runtime_command_schedule_task() {
        let mut runtime = Runtime::new();
        runtime.boot();

        let executed = Arc::new(AtomicBool::new(false));
        let executed_clone = Arc::clone(&executed);

        let task = Task::new(move || {
            executed_clone.store(true, Ordering::SeqCst);
        });

        let sender = runtime.command_sender();
        sender.send(RuntimeCommand::ScheduleTask(task)).unwrap();

        runtime.tick();
        runtime.tick(); // Need second tick to execute the scheduled task

        assert!(executed.load(Ordering::SeqCst));
    }

    // === Send test ===

    #[test]
    fn test_runtime_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Runtime>();
    }
}
