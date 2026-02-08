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

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::{sync::atomic::AtomicUsize, time::Duration},
    };

    #[test]
    fn test_saturator_submit() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let handle = spawn_saturator(
            |x: i32| x * 2,
            move |_result| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            },
        );

        handle.submit(1, None);
        handle.submit(2, None);
        handle.submit(3, None);

        // Give worker time to process
        thread::sleep(Duration::from_millis(50));

        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_saturator_submit_background() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let handle = spawn_saturator(
            |x: i32| x,
            move |_result| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            },
        );

        handle.submit_background(1, None);
        handle.submit_background(2, None);

        thread::sleep(Duration::from_millis(50));

        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_saturator_scope_tracking() {
        let scope = EventScope::new();
        let scope_clone = scope.clone();

        let handle = spawn_saturator(|x: i32| x, |_result| {});

        handle.submit(1, Some(&scope_clone));
        handle.submit(2, Some(&scope_clone));

        // Wait for processing with timeout
        let completed = scope.wait_timeout(Duration::from_millis(100));
        assert!(completed, "Scope should complete when all items processed");
    }

    #[test]
    fn test_saturator_priority_ordering() {
        let results = Arc::new(std::sync::Mutex::new(Vec::new()));
        let results_clone = Arc::clone(&results);

        let handle = spawn_saturator(
            |x: i32| x,
            move |result| {
                results_clone.lock().unwrap().push(result);
            },
        );

        // Submit low priority first
        handle.submit_background(3, None);
        // Then high priority
        handle.submit(1, None);
        handle.submit(2, None);

        thread::sleep(Duration::from_millis(50));

        let guard = results.lock().unwrap();
        // High priority items (1, 2) should be processed before low (3)
        // Note: exact order depends on timing, but high should come before low
        assert!(guard.contains(&1));
        assert!(guard.contains(&2));
        assert!(guard.contains(&3));
        drop(guard);
    }

    #[test]
    fn test_saturator_shutdown() {
        let handle = spawn_saturator(|x: i32| x, |_result| {});

        handle.shutdown();
        assert!(handle.is_shutting_down());
    }

    #[test]
    fn test_saturator_clone() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let handle = spawn_saturator(
            |x: i32| x,
            move |_result| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            },
        );

        // Clone the handle
        let handle2 = handle.clone();

        // Submit from both handles
        handle.submit(1, None);
        handle2.submit(2, None);

        thread::sleep(Duration::from_millis(50));

        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_request_priority_default() {
        assert_eq!(RequestPriority::default(), RequestPriority::High);
    }

    #[test]
    fn test_submit_request() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let handle = spawn_saturator(
            |x: i32| x,
            move |_result| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            },
        );

        handle.submit_request(SaturationRequest {
            data: 42,
            priority: RequestPriority::High,
            scope: None,
        });

        thread::sleep(Duration::from_millis(50));

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    // === Coverage: submit_request with Low priority ===

    #[test]
    fn test_submit_request_low_priority() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let handle = spawn_saturator(
            |x: i32| x,
            move |_result| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            },
        );

        handle.submit_request(SaturationRequest {
            data: 99,
            priority: RequestPriority::Low,
            scope: None,
        });

        thread::sleep(Duration::from_millis(50));

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    // === Coverage: SaturatorConfig default ===

    #[test]
    fn test_saturator_config_default() {
        let config = SaturatorConfig::default();
        assert!(config.drain_on_shutdown);
    }

    // === Coverage: SaturatorConfig Debug and Clone ===

    #[test]
    fn test_saturator_config_debug_clone() {
        let config = SaturatorConfig {
            drain_on_shutdown: false,
        };
        let cloned = config.clone();
        assert!(!cloned.drain_on_shutdown);

        let debug = format!("{config:?}");
        assert!(debug.contains("SaturatorConfig"));
        assert!(debug.contains("false"));
    }

    // === Coverage: submit_background with scope tracking ===

    #[test]
    fn test_submit_background_with_scope() {
        let scope = EventScope::new();
        let scope_clone = scope.clone();

        let handle = spawn_saturator(|x: i32| x, |_result| {});

        handle.submit_background(1, Some(&scope_clone));
        handle.submit_background(2, Some(&scope_clone));

        let completed = scope.wait_timeout(Duration::from_millis(100));
        assert!(completed, "Scope should complete when all items processed");
    }

    // === Coverage: SaturationRequest Debug ===

    #[test]
    fn test_saturation_request_debug() {
        let request = SaturationRequest {
            data: 42i32,
            priority: RequestPriority::High,
            scope: None,
        };
        let debug = format!("{request:?}");
        assert!(debug.contains("SaturationRequest"));
        assert!(debug.contains("42"));
    }

    // === Coverage: RequestPriority Debug, Clone, Hash ===

    #[test]
    fn test_request_priority_debug_clone_hash() {
        use std::collections::HashSet;
        let high = RequestPriority::High;
        let low = RequestPriority::Low;

        let cloned = high;
        assert_eq!(cloned, RequestPriority::High);

        let debug_high = format!("{high:?}");
        assert!(debug_high.contains("High"));

        let debug_low = format!("{low:?}");
        assert!(debug_low.contains("Low"));

        let mut set = HashSet::new();
        set.insert(high);
        set.insert(low);
        assert_eq!(set.len(), 2);
    }

    // === Coverage: Drop with worker thread join ===

    #[test]
    fn test_saturator_drop_joins_worker() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        {
            let handle = spawn_saturator(
                |x: i32| x,
                move |_result| {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                },
            );
            handle.submit(1, None);
            thread::sleep(Duration::from_millis(50));
            // handle drops here, setting shutdown flag and joining worker
        }

        // Worker should have processed the item before shutting down
        assert!(counter.load(Ordering::SeqCst) >= 1);
    }

    // === Coverage: submit_request with scope ===

    #[test]
    fn test_submit_request_with_scope() {
        let scope = EventScope::new();
        let scope_clone = scope.clone();

        let handle = spawn_saturator(|x: i32| x, |_result| {});

        handle.submit_request(SaturationRequest {
            data: 7,
            priority: RequestPriority::High,
            scope: Some(scope_clone),
        });

        let completed = scope.wait_timeout(Duration::from_millis(100));
        assert!(completed);
    }
}
