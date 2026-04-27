//! Background task scheduler for lazy computation.
//!
//! The Saturator provides a mechanism for scheduling background computation
//! with priority levels. It's designed for tasks like syntax highlighting
//! where viewport content should be processed immediately (high priority)
//! while off-screen content can be processed later (low priority).
//!
//! # Design Philosophy
//!
//! Following the kernel "mechanism, not policy" principle:
//! - Generic over work item type and result type
//! - No syntax or highlighting knowledge in the kernel
//! - Priority is a simple high/low distinction
//! - `EventScope` integration for lifecycle tracking
//!
//! # Thread Model
//!
//! The saturator spawns a dedicated background thread that processes work
//! items from two priority queues. High priority items are always processed
//! before low priority items.
//!
//! # Example
//!
//! ```ignore
//! use reovim_kernel::api::v1::*;
//!
//! // Create a saturator for processing lines
//! let saturator = spawn_saturator(
//!     |line_idx: usize| {
//!         // Process line and return result
//!         format!("Processed line {}", line_idx)
//!     },
//!     |result| {
//!         println!("Completed: {}", result);
//!     },
//! );
//!
//! // Submit high-priority work (viewport)
//! saturator.submit(0, None);
//! saturator.submit(1, None);
//!
//! // Submit low-priority work (off-screen)
//! saturator.submit_background(100, None);
//! ```

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

use crate::ipc::{EventScope, Receiver, Sender, channel};

/// Request priority for background computation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum RequestPriority {
    /// High priority - process immediately (viewport content).
    #[default]
    High,
    /// Low priority - process when idle (off-screen content).
    Low,
}

/// A request to the saturator with associated scope.
#[derive(Debug)]
pub struct SaturationRequest<T> {
    /// The work data to process.
    pub data: T,
    /// Request priority.
    pub priority: RequestPriority,
    /// Optional scope for lifecycle tracking.
    pub scope: Option<EventScope>,
}

/// Handle to send work to a saturator.
///
/// The handle is cheap to clone and can be shared across threads.
/// When all handles are dropped, the saturator will finish processing
/// remaining work and shut down.
pub struct SaturatorHandle<T> {
    /// Channel for high-priority work.
    high_tx: Sender<WorkItem<T>>,
    /// Channel for low-priority work.
    low_tx: Sender<WorkItem<T>>,
    /// Shutdown flag.
    shutdown: Arc<AtomicBool>,
    /// Worker thread handle (only the original handle has this).
    worker: Option<JoinHandle<()>>,
}

/// Internal work item with scope tracking.
struct WorkItem<T> {
    data: T,
    scope: Option<EventScope>,
}

impl<T: Send + 'static> SaturatorHandle<T> {
    /// Submit high-priority work with optional scope tracking.
    ///
    /// Follows the `EventScope` pattern from `event_bus.rs:252-259`:
    /// scope is incremented before submit and decremented after completion.
    ///
    /// # Arguments
    ///
    /// * `work` - The work data to process
    /// * `scope` - Optional scope for lifecycle tracking
    pub fn submit(&self, work: T, scope: Option<&EventScope>) {
        if let Some(s) = scope {
            s.increment();
        }
        let _ = self.high_tx.send(WorkItem {
            data: work,
            scope: scope.cloned(),
        });
    }

    /// Submit low-priority background work.
    ///
    /// # Arguments
    ///
    /// * `work` - The work data to process
    /// * `scope` - Optional scope for lifecycle tracking
    pub fn submit_background(&self, work: T, scope: Option<&EventScope>) {
        if let Some(s) = scope {
            s.increment();
        }
        let _ = self.low_tx.send(WorkItem {
            data: work,
            scope: scope.cloned(),
        });
    }

    /// Submit a request with explicit priority.
    pub fn submit_request(&self, request: SaturationRequest<T>) {
        let scope = request.scope.as_ref();
        match request.priority {
            RequestPriority::High => self.submit(request.data, scope),
            RequestPriority::Low => self.submit_background(request.data, scope),
        }
    }

    /// Request shutdown of the saturator.
    ///
    /// The worker will finish processing remaining items before stopping.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
    }

    /// Check if the saturator is shutting down.
    #[must_use]
    pub fn is_shutting_down(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }
}

impl<T> Clone for SaturatorHandle<T> {
    fn clone(&self) -> Self {
        Self {
            high_tx: self.high_tx.clone(),
            low_tx: self.low_tx.clone(),
            shutdown: Arc::clone(&self.shutdown),
            worker: None, // Clones don't own the worker thread
        }
    }
}

impl<T> Drop for SaturatorHandle<T> {
    fn drop(&mut self) {
        // Only the original handle owns the worker thread
        if let Some(worker) = self.worker.take() {
            self.shutdown.store(true, Ordering::Release);
            // Wait for worker to finish
            let _ = worker.join();
        }
    }
}

/// Spawn a saturator with the given processor and completion callback.
///
/// Creates a background worker thread that processes work items and calls
/// the completion callback with results.
///
/// # Arguments
///
/// * `processor` - Function to process work items
/// * `on_complete` - Callback called with each result
///
/// # Type Parameters
///
/// * `T` - Work item type (must be Send + 'static)
/// * `F` - Processor function type
/// * `R` - Result type (must be Send + 'static)
///
/// # Returns
///
/// A handle for submitting work to the saturator.
///
/// # Example
///
/// ```ignore
/// let handle = spawn_saturator(
///     |x: i32| x * 2,
///     |result| println!("Result: {}", result),
/// );
///
/// handle.submit(21, None);  // Will print "Result: 42"
/// ```
pub fn spawn_saturator<T, F, R, C>(processor: F, on_complete: C) -> SaturatorHandle<T>
where
    T: Send + 'static,
    F: Fn(T) -> R + Send + Sync + 'static,
    R: Send + 'static,
    C: Fn(R) + Send + Sync + 'static,
{
    let (high_tx, high_rx) = channel();
    let (low_tx, low_rx) = channel();
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown);

    let processor = Arc::new(processor);
    let on_complete = Arc::new(on_complete);

    let worker = thread::spawn(move || {
        worker_loop(high_rx, low_rx, shutdown_clone, processor, on_complete);
    });

    SaturatorHandle {
        high_tx,
        low_tx,
        shutdown,
        worker: Some(worker),
    }
}

/// Worker loop that processes items from both queues.
#[allow(clippy::needless_pass_by_value)] // Receivers are intentionally moved into the thread
#[cfg_attr(coverage_nightly, coverage(off))]
fn worker_loop<T, F, R, C>(
    high_rx: Receiver<WorkItem<T>>,
    low_rx: Receiver<WorkItem<T>>,
    shutdown: Arc<AtomicBool>,
    processor: Arc<F>,
    on_complete: Arc<C>,
) where
    T: Send + 'static,
    F: Fn(T) -> R + Send + Sync + 'static,
    R: Send + 'static,
    C: Fn(R) + Send + Sync + 'static,
{
    loop {
        // Check shutdown flag
        if shutdown.load(Ordering::Acquire) {
            // Drain remaining high-priority items before shutting down
            while let Ok(item) = high_rx.try_recv() {
                process_item(item, &processor, &on_complete);
            }
            break;
        }

        // Try high priority first (biased polling)
        if let Ok(item) = high_rx.try_recv() {
            process_item(item, &processor, &on_complete);
            continue;
        }

        // Then try low priority
        if let Ok(item) = low_rx.try_recv() {
            process_item(item, &processor, &on_complete);
            continue;
        }

        // No work available, yield to avoid busy-waiting
        thread::yield_now();
    }
}

/// Process a single work item.
fn process_item<T, F, R, C>(item: WorkItem<T>, processor: &Arc<F>, on_complete: &Arc<C>)
where
    F: Fn(T) -> R,
    C: Fn(R),
{
    let result = processor(item.data);
    on_complete(result);

    // Decrement scope after completion (follows event_bus.rs pattern)
    if let Some(scope) = item.scope {
        scope.decrement();
    }
}

/// Configuration for creating a saturator.
#[derive(Debug, Clone)]
pub struct SaturatorConfig {
    /// Whether to process remaining items on shutdown.
    pub drain_on_shutdown: bool,
}

impl Default for SaturatorConfig {
    fn default() -> Self {
        Self {
            drain_on_shutdown: true,
        }
    }
}
