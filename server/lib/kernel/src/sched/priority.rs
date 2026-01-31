//! Priority queue for ordered event processing.
//!
//! Linux equivalent: Priority scheduling in `kernel/sched/`
//!
//! This module provides a priority-ordered queue for events. Events with
//! lower priority values are processed first. Events with the same priority
//! are processed in FIFO order.
//!
//! # Example
//!
//! ```ignore
//! use reovim_kernel::sched::PriorityQueue;
//! use reovim_kernel::ipc::{DynEvent, Event};
//!
//! #[derive(Debug)]
//! struct HighPriorityEvent;
//! impl Event for HighPriorityEvent {
//!     fn priority(&self) -> u32 { 0 }
//! }
//!
//! #[derive(Debug)]
//! struct LowPriorityEvent;
//! impl Event for LowPriorityEvent {
//!     fn priority(&self) -> u32 { 200 }
//! }
//!
//! let queue = PriorityQueue::new();
//!
//! // Add events in reverse priority order
//! queue.push(DynEvent::new(LowPriorityEvent));
//! queue.push(DynEvent::new(HighPriorityEvent));
//!
//! // Pop returns highest priority (lowest value) first
//! let event = queue.pop().unwrap();
//! assert_eq!(event.priority(), 0); // HighPriorityEvent
//! ```

use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
};

use crate::{arch::sync::Mutex, ipc::DynEvent};

/// Default capacity for priority queue.
pub const DEFAULT_CAPACITY: usize = 256;

/// Entry in the priority queue.
///
/// Wraps a `DynEvent` with ordering metadata for the binary heap.
struct PriorityEntry {
    /// Event priority (from `Event::priority()`).
    priority: u32,

    /// Insertion sequence for FIFO ordering within same priority.
    sequence: u64,

    /// The event.
    event: DynEvent,
}

impl PartialEq for PriorityEntry {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.sequence == other.sequence
    }
}

impl Eq for PriorityEntry {}

impl PartialOrd for PriorityEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriorityEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap, so we reverse the ordering:
        // - Lower priority value = higher precedence (comes first)
        // - For same priority, lower sequence = earlier (FIFO)
        //
        // We want: smaller priority → "greater" in heap terms
        // We want: smaller sequence → "greater" in heap terms (for same priority)
        match other.priority.cmp(&self.priority) {
            Ordering::Equal => other.sequence.cmp(&self.sequence),
            ord => ord,
        }
    }
}

/// Priority queue for events.
///
/// Maintains events in priority order with FIFO ordering for events
/// of the same priority. Thread-safe via internal mutex.
///
/// # Ordering
///
/// Events are ordered by:
/// 1. Priority (lower value = higher precedence)
/// 2. Insertion order (earlier = higher precedence)
///
/// This ensures that high-priority events (like user input) are always
/// processed before low-priority events (like render signals).
pub struct PriorityQueue {
    /// Heap storage.
    heap: Mutex<BinaryHeap<PriorityEntry>>,

    /// Sequence counter for FIFO ordering.
    sequence: AtomicU64,

    /// Maximum capacity (0 = unlimited).
    capacity: usize,
}

impl PriorityQueue {
    /// Create a new priority queue with default capacity.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    /// Create a priority queue with specified capacity.
    ///
    /// A capacity of 0 means unlimited (not recommended).
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            heap: Mutex::new(BinaryHeap::with_capacity(capacity.min(4096))),
            sequence: AtomicU64::new(0),
            capacity,
        }
    }

    /// Push an event onto the queue.
    ///
    /// Returns `false` if capacity is exceeded (event is dropped).
    pub fn push(&self, event: DynEvent) -> bool {
        let mut heap = self.heap.lock();

        if self.capacity > 0 && heap.len() >= self.capacity {
            return false;
        }

        let entry = PriorityEntry {
            priority: event.priority(),
            sequence: self.sequence.fetch_add(1, AtomicOrdering::Relaxed),
            event,
        };
        heap.push(entry);
        true
    }

    /// Pop the highest priority event.
    ///
    /// Returns `None` if the queue is empty.
    #[must_use]
    pub fn pop(&self) -> Option<DynEvent> {
        self.heap.lock().pop().map(|e| e.event)
    }

    /// Peek at the priority of the next event without removing it.
    ///
    /// Returns `None` if the queue is empty.
    #[must_use]
    pub fn peek_priority(&self) -> Option<u32> {
        self.heap.lock().peek().map(|e| e.priority)
    }

    /// Get the number of events in the queue.
    #[must_use]
    pub fn len(&self) -> usize {
        self.heap.lock().len()
    }

    /// Check if the queue is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.heap.lock().is_empty()
    }

    /// Get the queue capacity.
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clear all events from the queue.
    pub fn clear(&self) {
        self.heap.lock().clear();
    }

    /// Drain all events from the queue in priority order.
    ///
    /// Returns events sorted by priority (highest first).
    #[must_use]
    pub fn drain(&self) -> Vec<DynEvent> {
        let mut heap = self.heap.lock();
        let mut result = Vec::with_capacity(heap.len());
        while let Some(entry) = heap.pop() {
            result.push(entry.event);
        }
        result
    }
}

impl Default for PriorityQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for PriorityQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PriorityQueue")
            .field("len", &self.len())
            .field("capacity", &self.capacity)
            .field("peek_priority", &self.peek_priority())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::ipc::Event};

    // Test event types with different priorities
    #[derive(Debug)]
    struct CriticalEvent;
    impl Event for CriticalEvent {
        fn priority(&self) -> u32 {
            0
        }
    }

    #[derive(Debug)]
    struct HighEvent;
    impl Event for HighEvent {
        fn priority(&self) -> u32 {
            50
        }
    }

    #[derive(Debug)]
    struct NormalEvent(u32);
    impl Event for NormalEvent {
        fn priority(&self) -> u32 {
            100
        }
    }

    #[derive(Debug)]
    struct LowEvent;
    impl Event for LowEvent {
        fn priority(&self) -> u32 {
            200
        }
    }

    // === Basic tests ===

    #[test]
    fn test_new() {
        let queue = PriorityQueue::new();
        assert_eq!(queue.capacity(), DEFAULT_CAPACITY);
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_with_capacity() {
        let queue = PriorityQueue::with_capacity(100);
        assert_eq!(queue.capacity(), 100);
    }

    #[test]
    fn test_push_and_pop() {
        let queue = PriorityQueue::new();
        queue.push(DynEvent::new(NormalEvent(1)));
        assert_eq!(queue.len(), 1);

        let event = queue.pop().unwrap();
        assert_eq!(event.priority(), 100);
        assert!(queue.is_empty());
    }

    // === Priority ordering tests ===

    #[test]
    fn test_priority_ordering() {
        let queue = PriorityQueue::new();

        // Add events in reverse priority order
        queue.push(DynEvent::new(LowEvent));
        queue.push(DynEvent::new(NormalEvent(1)));
        queue.push(DynEvent::new(HighEvent));
        queue.push(DynEvent::new(CriticalEvent));

        // Pop should return in priority order (lowest value first)
        assert_eq!(queue.pop().unwrap().priority(), 0); // Critical
        assert_eq!(queue.pop().unwrap().priority(), 50); // High
        assert_eq!(queue.pop().unwrap().priority(), 100); // Normal
        assert_eq!(queue.pop().unwrap().priority(), 200); // Low
    }

    #[test]
    fn test_fifo_same_priority() {
        let queue = PriorityQueue::new();

        // Add multiple events with same priority
        for i in 0..5 {
            queue.push(DynEvent::new(NormalEvent(i)));
        }

        // Should pop in FIFO order
        for i in 0..5 {
            let event = queue.pop().unwrap();
            let inner = event.downcast_ref::<NormalEvent>().unwrap();
            assert_eq!(inner.0, i);
        }
    }

    #[test]
    fn test_mixed_priority_fifo() {
        let queue = PriorityQueue::new();

        // Add interleaved events
        queue.push(DynEvent::new(NormalEvent(1)));
        queue.push(DynEvent::new(HighEvent));
        queue.push(DynEvent::new(NormalEvent(2)));
        queue.push(DynEvent::new(LowEvent));
        queue.push(DynEvent::new(NormalEvent(3)));

        // High priority first
        assert_eq!(queue.pop().unwrap().priority(), 50);

        // Then normal priority in FIFO order
        for expected in [1, 2, 3] {
            let event = queue.pop().unwrap();
            let inner = event.downcast_ref::<NormalEvent>().unwrap();
            assert_eq!(inner.0, expected);
        }

        // Then low priority
        assert_eq!(queue.pop().unwrap().priority(), 200);
    }

    // === Peek tests ===

    #[test]
    fn test_peek_priority() {
        let queue = PriorityQueue::new();
        assert!(queue.peek_priority().is_none());

        queue.push(DynEvent::new(LowEvent));
        assert_eq!(queue.peek_priority(), Some(200));

        queue.push(DynEvent::new(HighEvent));
        assert_eq!(queue.peek_priority(), Some(50));

        // Peek shouldn't remove
        assert_eq!(queue.len(), 2);
    }

    // === Capacity tests ===

    #[test]
    fn test_capacity_overflow() {
        let queue = PriorityQueue::with_capacity(3);

        assert!(queue.push(DynEvent::new(NormalEvent(1))));
        assert!(queue.push(DynEvent::new(NormalEvent(2))));
        assert!(queue.push(DynEvent::new(NormalEvent(3))));
        assert!(!queue.push(DynEvent::new(NormalEvent(4)))); // Should fail

        assert_eq!(queue.len(), 3);
    }

    // === Drain tests ===

    #[test]
    fn test_drain() {
        let queue = PriorityQueue::new();

        queue.push(DynEvent::new(LowEvent));
        queue.push(DynEvent::new(HighEvent));
        queue.push(DynEvent::new(NormalEvent(1)));

        let events = queue.drain();
        assert_eq!(events.len(), 3);
        assert!(queue.is_empty());

        // Should be in priority order
        assert_eq!(events[0].priority(), 50);
        assert_eq!(events[1].priority(), 100);
        assert_eq!(events[2].priority(), 200);
    }

    // === Clear tests ===

    #[test]
    fn test_clear() {
        let queue = PriorityQueue::new();

        for _ in 0..5 {
            queue.push(DynEvent::new(NormalEvent(0)));
        }

        queue.clear();
        assert!(queue.is_empty());
    }

    // === Debug tests ===

    #[test]
    fn test_debug() {
        let queue = PriorityQueue::with_capacity(100);
        queue.push(DynEvent::new(NormalEvent(1)));
        let debug_str = format!("{queue:?}");
        assert!(debug_str.contains("PriorityQueue"));
        assert!(debug_str.contains("len"));
    }

    // === Send + Sync tests ===

    #[test]
    fn test_priority_queue_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PriorityQueue>();
    }

    // === Concurrent tests ===

    #[test]
    fn test_concurrent_push() {
        use std::{sync::Arc, thread};

        let queue = Arc::new(PriorityQueue::with_capacity(1000));
        let mut handles = vec![];

        for _ in 0..10 {
            let queue_clone = Arc::clone(&queue);
            handles.push(thread::spawn(move || {
                for _ in 0..50 {
                    queue_clone.push(DynEvent::new(NormalEvent(0)));
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(queue.len(), 500);
    }
}
