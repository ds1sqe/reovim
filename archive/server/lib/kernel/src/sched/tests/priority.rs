use {
    super::super::*,
    crate::ipc::{DynEvent, Event},
};

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
    assert_eq!(queue.capacity(), PRIORITY_QUEUE_DEFAULT_CAPACITY);
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
fn test_default() {
    let queue = PriorityQueue::default();
    assert_eq!(queue.capacity(), PRIORITY_QUEUE_DEFAULT_CAPACITY);
    assert!(queue.is_empty());
}

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
